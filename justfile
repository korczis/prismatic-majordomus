# Majordomus — the recipes a person runs here. `just` lists them; `just <recipe>` runs one.
#
# What is *not* in this file: a recipe for anything the Rust executable already declares.
# Those are a projection of the canonical command graph, generated into an ignored runtime
# file and imported below, with their descriptions, their groups, their compatibility
# aliases and their confirmations all derived from the command's own declaration. Adding a
# command to apps/majordomus-cli therefore adds a recipe here, and editing this file to add
# one by hand would be declaring it twice. `just bridge` materialises the projection;
# `majordomus commands explain <id>` says where any of it came from.
#
# What is in this file: bootstrap that cannot be derived (building the executable), the
# other executable — the shell tool `bin/majordomus`, whose task lifecycle is its own
# program — and the workflows that are genuinely scripts. Every recipe is a thin call: the
# source of truth for what a step does is the script or the command it names, never this
# file.

set shell := ["bash", "-euo", "pipefail", "-c"]

root      := justfile_directory()
crate     := root / "apps/majordomus-cli"
manifest  := crate / "Cargo.toml"
profile   := env("MAJORDOMUS_BUILD_PROFILE", "debug")
rust_bin  := crate / "target" / profile / "majordomus"
shell_bin := root / "bin/majordomus"

export MAJORDOMUS_SHARE := root / "share"

# The generated bridge: one recipe per canonical command of the Rust executable. Optional,
# because a fresh clone has not materialised it yet and `just --list` must still work;
# `just bridge` writes it, and anything that needs it depends on that recipe.
import? ".majordomus/runtime/just/bridge.just"

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

# Materialise the generated `just` bridge: one recipe per canonical command, written under the ignored .majordomus/runtime/ only when its bytes would differ. Run it after a fresh clone, and after adding a command.
[group('build')]
bridge: build
    "{{rust_bin}}" commands materialise

# Remove the Rust build output.
[group('build')]
[confirm("Remove apps/majordomus-cli/target? [y/N]")]
clean:
    cargo clean --manifest-path "{{manifest}}"

# ---------------------------------------------------------------- serve (Rust executable)

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
swagger-ui:
    @f=".ai/local/state/mcp/server.json"; [ -f "$f" ] || { echo "no shared server is running (just serve, or start an MCP client)"; exit 1; }; \
    url="$(sed -n 's/.*"url":"\([^"]*\)".*/\1/p' "$f")"; echo "$url/swagger"; (command -v open >/dev/null && open "$url/swagger") || xdg-open "$url/swagger"

# Open the running shared server's home page — every surface it serves, derived from the topology.
[group('serve')]
home:
    @f=".ai/local/state/mcp/server.json"; [ -f "$f" ] || { echo "no shared server is running (just serve, or start an MCP client)"; exit 1; }; \
    url="$(sed -n 's/.*"url":"\([^"]*\)".*/\1/p' "$f")"; echo "$url/"; (command -v open >/dev/null && open "$url/") || xdg-open "$url/"

# Open the Cockpit of the running shared server in the browser (macOS `open`, else xdg-open).
[group('serve')]
cockpit:
    @f=".ai/local/state/mcp/server.json"; [ -f "$f" ] || { echo "no shared server is running (just serve, or start an MCP client)"; exit 1; }; \
    url="$(sed -n 's/.*"url":"\([^"]*\)".*/\1/p' "$f")"; echo "$url/cockpit"; (command -v open >/dev/null && open "$url/cockpit") || xdg-open "$url/cockpit"

# Build the Cockpit's static assets: compile share/cockpit/cockpit.css, vendor the pinned libraries (needs npm ci).
[group('serve')]
cockpit-assets:
    scripts/cockpit-assets

# The committed Cockpit stylesheet matches its source; exit 10 when it is stale.
[group('serve')]
cockpit-assets-check:
    scripts/cockpit-assets --check

# Every Cockpit route the server serves, answered; then the pages and the interactions in a real browser.
[group('serve')]
cockpit-probe *args:
    scripts/cockpit-probe {{args}}

# ---------------------------------------------------------------- registry (Rust executable)

# Exit 10 naming every stale generated projection; writes nothing.
[group('registry')]
generate-check: build
    "{{rust_bin}}" generate --check

# ---------------------------------------------------------------- benchmarks (Rust executable)

# Compare a run with this platform's accepted baseline under .ai/repo/benchmarks/rust/ (policy.yaml); exit 10 on a regression.
[group('bench')]
bench-check *args: build
    "{{rust_bin}}" bench --profile ci --check --no-write {{args}}

# ---------------------------------------------------------------- use cases (shell tool)

# Every use case: id, category, status, whether it has a scenario, the commands it runs.
[group('use-cases')]
usecase-list *args:
    bin/majordomus usecase list {{args}}

# Execute every scenario against the real tool in disposable repositories; evidence under .ai/local/evidence/use-cases/.
[group('use-cases')]
usecase-run *args:
    bin/majordomus usecase run {{args}}

# Every public command, guaranteed claim and MCP tool against the use cases that name and run it; exit 10 on a required gap.
[group('use-cases')]
usecase-coverage *args:
    bin/majordomus usecase coverage --check {{args}}

# What a change reaches: the commands, rules, use cases, scenarios and cases, from the files changed since the upstream.
[group('use-cases')]
usecase-impact *args:
    bin/majordomus usecase impact {{args}}

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

# The criterion microbenchmarks (benches/projections.rs, benches/shared.rs, benches/scaling.rs). `just bench-criterion scaling` runs one; `just bench` is the end-to-end measurement of the executable itself, bridged from the command graph.
[group('test')]
bench-criterion *name:
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

# Deploy the site by hand: gate, build, check, push gh-pages (see .ai/repo/skills/deploy-site/SKILL.md). `just site-deploy --dry-run` shows what it would push.
[group('site')]
site-deploy *args:
    scripts/site-deploy {{args}}

# Build the website (zola).
[group('site')]
site-build:
    scripts/site-build

# Build the same documentation source for the running executable: mounted at /docs, written
# into target/web/docs with the surface.json that makes it discoverable. One source, one
# generator, two base URLs; `just serve` then answers /docs/ from it.
[group('site')]
site-docs:
    scripts/site-build --serve

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

# The publication fast path, exactly as .github/workflows/pages.yml runs it: prove the committed derived data current by its input hash, render it, check the output. `just pages build`, `pages check`, `pages paths`, `pages budget`, `pages current`, `pages verify`.
[group('site')]
pages *args:
    scripts/pages {{args}}

# The controlled publication path, measured N times; `just pages-benchmark -n 10 --write-baseline` records this platform's baseline under .ai/repo/benchmarks/pages/.
[group('site')]
pages-benchmark *args:
    scripts/pages benchmark {{args}}

# Every committed derived artifact, regenerated in dependency order (scripts/derive: generate, site data, generate again over the index the site data changed).
[group('site')]
derive:
    scripts/derive

# Exit 10 naming every stale derived artifact — the registry's projections and the site's data — and write nothing (scripts/derive-check).
[group('site')]
derive-check:
    scripts/derive-check

# Declare the `derived` merge driver this clone needs, so .gitattributes resolves the derived artifacts on merge instead of conflicting on their fingerprints (scripts/merge-derived).
[group('site')]
derive-merge-driver:
    git config merge.derived.name "derived artifacts: resolve to ours, regenerate before committing"
    git config merge.derived.driver "{{root}}/scripts/merge-derived %O %A %B %P"
    @echo "merge.derived wired; .gitattributes now resolves the derived artifacts on merge"

# ---------------------------------------------------------------- worktree (Rust executable)

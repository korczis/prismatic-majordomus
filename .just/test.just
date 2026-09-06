# The gates. `just test` is the whole of what CI runs for a change to this repository;
# the rest narrow it to one suite while you are working on that suite.
#
# `test` is the entry point of this group, which is what puts `just test` on the banner:
# the environment snapshot offers the public recipe whose name is its group's name, and
# offers nothing when there is none.

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

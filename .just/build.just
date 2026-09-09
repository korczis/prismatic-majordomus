# Building the Rust executable. Everything under `serve`, `registry`, `bench` and `env`
# depends on `build`, so a contributor never has to remember to run it.

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

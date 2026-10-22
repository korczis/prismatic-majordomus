# Building the Rust executable. Everything under `serve`, `registry`, `bench` and `env`
# depends on `build`, so a contributor never has to remember to run it.

# Build the Rust executable (debug; MAJORDOMUS_BUILD_PROFILE=release for a release build).
#
# The build is bounded on the disk it may consume before it starts, the way every wait here
# is bounded on time: 2.45GB of target/ per build, and a machine that reaches zero bytes free
# takes every session on it down at once. The floor and its measurement are in
# lib/rust_bin.sh; MAJORDOMUS_MIN_FREE_MB=0 lifts it.
[group('build')]
build:
    source "{{root}}/lib/rust_bin.sh" && mj_rust_space_check "{{root}}" "just build"
    RUSTFLAGS='' cargo build --locked --manifest-path "{{manifest}}" {{ if profile == "release" { "--release" } else { "" } }}

# Build the release executable.
[group('build')]
build-release:
    source "{{root}}/lib/rust_bin.sh" && mj_rust_space_check "{{root}}" "just build-release"
    RUSTFLAGS='' cargo build --locked --release --manifest-path "{{manifest}}"

# Remove the Rust build output of THIS checkout (refuses to remove another checkout's).
#
# `cargo clean --manifest-path X` does not clean X's target directory: with CARGO_TARGET_DIR
# set it cleans that instead, and the manifest decides nothing — which is how a worker
# reclaiming their own worktree removed the build directory several other worktrees were
# sharing, twice in one day, while their own target/ survived and the prompt they answered
# named a path under their own checkout. The recipe cannot name the directory here either:
# the prompt is a literal and the answer is only known at run time. So the decision is
# lib/rust_bin.sh's, which resolves where cargo would really clean, refuses when that is
# outside this checkout and says why, and otherwise hands cargo the directory explicitly.
[group('build')]
[confirm("Remove this checkout's Rust build directory? [y/N]")]
clean:
    source "{{root}}/lib/rust_bin.sh" && mj_rust_clean "{{root}}"

# Where the Rust executable of this repository is.
#
# One question, asked by two callers with different answers to the *next* question:
# `bin/majordomus-mcp` may build it when it is missing, because an MCP client spawning a
# server can afford a build; `bin/majordomus-env` may never build it, because it runs on
# every `cd` and a shell prompt cannot afford one. Both need the same path, so the path is
# decided here and the build policy is decided by each caller.
#
# Order: an explicit MAJORDOMUS_BIN, then the executable an installed release ships beside
# bin/, then the crate's target directory, which is what a contributor has.
#
#   MAJORDOMUS_BIN            use this executable
#   MAJORDOMUS_BUILD_PROFILE  debug (default) or release
#   CARGO_TARGET_DIR          where cargo builds; read, never assumed
#
# Sourced by scripts that are not the shell tool, so it defines functions and sets nothing.

# mj_cargo_target_dir <crate-directory> [ask-cargo]
#
# Where cargo writes that crate's build outputs. Composing "<crate>/target" is a guess, and
# it is wrong the moment CARGO_TARGET_DIR is set — which is how several worktrees of this
# repository share one multi-gigabyte build directory instead of each carrying its own, and
# the only way a worktree survives another session deleting apps/majordomus-cli/target under
# it. Cargo then builds into the shared directory and a caller that composed the literal
# path runs an executable that was never there: scripts/derive died exactly that way, exit
# 127 on a path that does not exist.
#
# The environment is read first because it costs nothing and it is what such a session
# actually sets. `cargo metadata --format-version 1 --no-deps` is the authoritative answer —
# it folds in a build.target-dir from any .cargo/config.toml as well — but it is a whole
# process, ~80ms here, so it is asked only when the caller passes `ask-cargo`. The callers
# that do not are the ones that cannot: bin/majordomus-env runs on every `cd`, and
# test/lib.sh's rust_bin runs once per test case. Neither can spend 80ms to be told what an
# unset variable already said.
#
# One `target_directory` exists in that JSON, so the greedy match finds it without jq, which
# this file may not assume: it is sourced on the shell-prompt path.
mj_cargo_target_dir() {
  mj_ctd_crate="$1"
  if [ -n "${CARGO_TARGET_DIR:-}" ]; then printf '%s\n' "$CARGO_TARGET_DIR"; return 0; fi
  if [ -n "${CARGO_BUILD_TARGET_DIR:-}" ]; then printf '%s\n' "$CARGO_BUILD_TARGET_DIR"; return 0; fi
  if [ "${2:-}" = ask-cargo ] && command -v cargo >/dev/null 2>&1; then
    mj_ctd_dir="$(cargo metadata --format-version 1 --no-deps \
      --manifest-path "$mj_ctd_crate/Cargo.toml" 2>/dev/null \
      | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
    if [ -n "$mj_ctd_dir" ]; then printf '%s\n' "$mj_ctd_dir"; return 0; fi
  fi
  printf '%s\n' "$mj_ctd_crate/target"
}

# mj_rust_bin <repository-root>
mj_rust_bin() {
  mj_rb_root="$1"
  if [ -n "${MAJORDOMUS_BIN:-}" ]; then
    printf '%s\n' "$MAJORDOMUS_BIN"
    return 0
  fi
  mj_rb_shipped="$mj_rb_root/libexec/majordomus-cli"
  if [ -x "$mj_rb_shipped" ]; then
    printf '%s\n' "$mj_rb_shipped"
    return 0
  fi
  # no `ask-cargo`: this runs on every `cd` through bin/majordomus-env
  mj_rb_target="$(mj_cargo_target_dir "$mj_rb_root/apps/majordomus-cli")"
  printf '%s\n' "$mj_rb_target/${MAJORDOMUS_BUILD_PROFILE:-debug}/majordomus"
}

# The data directory the executable reads kinds and schemas from at run time, when this
# tree ships one. Empty when it does not, so a caller can leave MAJORDOMUS_SHARE unset and
# let the executable find its own.
mj_rust_share() {
  mj_rs_root="$1"
  if [ -n "${MAJORDOMUS_SHARE:-}" ]; then
    printf '%s\n' "$MAJORDOMUS_SHARE"
  elif [ -f "$mj_rs_root/share/kinds.yaml" ]; then
    printf '%s\n' "$mj_rs_root/share"
  fi
}

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
#
# Sourced by scripts that are not the shell tool, so it defines one function and sets
# nothing.

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
  printf '%s\n' "$mj_rb_root/apps/majordomus-cli/target/${MAJORDOMUS_BUILD_PROFILE:-debug}/majordomus"
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

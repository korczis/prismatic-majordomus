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

# mj_rust_stale <repository-root> <executable>
#
# Is that executable missing, or older than any source, manifest or lock file of the crate?
# True (0) for either. One question with two answers again: `bin/majordomus-cli` builds when
# this is true, `bin/majordomus-env` only says so — but neither may decide it for itself,
# because a checkout where the two disagree is a checkout where entering the repository
# reports one thing and running a command does another.
#
# An explicit MAJORDOMUS_BIN is never judged stale *here*: whoever named a particular
# executable owns whether it is current, and it need not sit beside sources at all. Neither
# is a tree without the crate — an installed release has no sources to be older than.
#
# That trust is about mtime, and it stops there. It does not extend to whether the named
# executable is of this tree's *generation*, because it cannot: a binary built minutes ago
# from an older revision of the crate is newer than every source and still carries another
# model. On 2026-09-10 exactly that binary, handed over through MAJORDOMUS_BIN, rewrote a
# pristine origin/master and removed 16,625 lines without a diagnostic. The executable
# decides that question itself — `majordomus generate` compares the generation compiled
# into it against the crate of the repository it is asked to derive and refuses with exit
# 15 — so every caller of this file inherits the guard and no shell script has to carry a
# second, drifting copy of it.
#
# Why this matters beyond a rebuild: every surface of this repository — the banner, the
# workflow bridge, the completion, the shared MCP server — is a projection of this one
# file. When it is old, all of them are old together, and nothing in their output says so.
# Silence here reads as a feature that vanished.
mj_rust_stale() {
  mj_st_root="$1"
  mj_st_bin="$2"
  [ -x "$mj_st_bin" ] || return 0
  [ -z "${MAJORDOMUS_BIN:-}" ] || return 1
  mj_st_crate="$mj_st_root/apps/majordomus-cli"
  [ -d "$mj_st_crate/src" ] || return 1
  mj_st_newer="$(
    find "$mj_st_crate/src" "$mj_st_crate/Cargo.toml" "$mj_st_crate/Cargo.lock" \
      -type f -newer "$mj_st_bin" 2>/dev/null | head -n 1
  )"
  [ -n "$mj_st_newer" ]
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

# mj_rust_target_named_by <repository-root>
#
# What decided where cargo builds, in the words a person can act on. The answer matters
# because the deciding thing is almost never the command that was typed: a variable exported
# in a shell two hours ago, a symlink laid down to share one build directory between several
# worktrees, or a config file, all silently move the build directory somewhere other than
# under the checkout the caller is standing in.
mj_rust_target_named_by() {
  mj_tn_root="$1"
  if [ -n "${CARGO_TARGET_DIR:-}" ]; then printf 'CARGO_TARGET_DIR\n'
  elif [ -n "${CARGO_BUILD_TARGET_DIR:-}" ]; then printf 'CARGO_BUILD_TARGET_DIR\n'
  elif [ -L "$mj_tn_root/apps/majordomus-cli/target" ]; then printf 'a symlink at apps/majordomus-cli/target\n'
  else printf "build.target-dir in a .cargo/config.toml\n"; fi
}

# mj_rust_physical <path>
#
# The path with every symlink resolved, without realpath(1) or readlink -f: this file is
# sourced on the shell-prompt path and macOS has neither. A directory only, which is all any
# caller here asks about. Prints nothing and fails when the path is not a directory.
mj_rust_physical() {
  [ -d "$1" ] || return 1
  ( unset CDPATH; cd -P -- "$1" >/dev/null 2>&1 && pwd -P ) || return 1
}

# mj_rust_owns <repository-root> <path>
#
# Is that directory inside that checkout, following symlinks on both sides? True (0) when it
# is. A symlink under the checkout that points outside it is *not* owned: the bytes it names
# are somebody else's, and this question is asked before deleting them.
mj_rust_owns() {
  mj_ow_root="$(mj_rust_physical "$1")" || return 1
  mj_ow_path="$(mj_rust_physical "$2")" || return 1
  case "$mj_ow_path/" in
    "$mj_ow_root"/*) return 0 ;;
    *) return 1 ;;
  esac
}

# mj_rust_outside <repository-root> <path-of-a-file>
#
# Is that file outside the checkout — a path this checkout does not own, and therefore one
# whose owner may remove it at any moment? Physically while the directory it would sit in
# still exists, and by the path itself when it does not, which is the usual case when the
# whole build directory is what went missing.
mj_rust_outside() {
  mj_ou_dir="$(dirname "$2")"
  if [ -d "$mj_ou_dir" ]; then
    mj_rust_owns "$1" "$mj_ou_dir" && return 1
    return 0
  fi
  case "$2" in "$1"/*) return 1 ;; *) return 0 ;; esac
}

# mj_rust_clean <repository-root>
#
# Remove the build directory this checkout builds into — and refuse when that directory is
# not this checkout's to remove.
#
# `cargo clean --manifest-path X` does not clean X's target directory: it cleans
# CARGO_TARGET_DIR whenever that is set, and the manifest then decides nothing at all. That
# is not a quirk to route around, it is the whole defect: several worktrees of this
# repository share one build directory by exactly that variable, so the one recipe a worker
# runs to reclaim their own disk is the one that takes everybody else's executable with it —
# while the confirmation prompt they answered named a path under their own checkout, and
# while their own target/ survives untouched. What the borrowers then see is their binary
# gone mid-run, which reads as a defect in whatever branch they were measuring.
#
# So: resolve where cargo would actually clean, refuse if that is outside this checkout, and
# otherwise name the directory to cargo explicitly, so that what is removed and what was
# announced are the same directory. Deliberately cleaning somebody else's is still possible;
# it just cannot be done by accident, because it takes typing their path.
mj_rust_clean() {
  mj_rc_root="$1"
  mj_rc_crate="$mj_rc_root/apps/majordomus-cli"
  mj_rc_target="$(mj_cargo_target_dir "$mj_rc_crate" ask-cargo)"

  if [ ! -d "$mj_rc_target" ]; then
    printf 'majordomus: nothing to remove: %s does not exist\n' "$mj_rc_target" >&2
    return 0
  fi

  if ! mj_rust_owns "$mj_rc_root" "$mj_rc_target"; then
    mj_rc_real="$(mj_rust_physical "$mj_rc_target")"
    mj_rc_named="$(mj_rust_target_named_by "$mj_rc_root")"
    {
      printf 'majordomus: refusing to clean: that build directory is not this checkout'"'"'s to remove.\n'
      # both paths physical, so that a reader comparing them is comparing the same kind of
      # thing: it is the boundary between them that the refusal is about
      printf '  this checkout:  %s\n' "$(mj_rust_physical "$mj_rc_root")"
      printf '  would remove:   %s\n' "$mj_rc_real"
      printf '  named by:       %s\n' "$mj_rc_named"
      printf '  Another checkout owns that directory and other workers may be building and running\n'
      printf '  out of it right now; removing it makes their executable vanish mid-run, which reads\n'
      printf '  to them as a defect in whatever branch they were measuring.\n'
      # the way out depends on what put the build directory there, and only one of these is
      # ever the right advice: telling somebody to unset a variable they never set is noise
      case "$mj_rc_named" in
        CARGO_*TARGET_DIR)
          printf '  To clean your own:            env -u %s just clean\n' "$mj_rc_named" ;;
        a\ symlink*)
          printf '  To clean your own:            remove the symlink at %s/target first,\n' "$mj_rc_crate"
          printf '                                so that this checkout builds its own again\n' ;;
        *)
          printf '  To clean your own:            unset build.target-dir, or clean where it points\n' ;;
      esac
      printf '  If you really mean that one:  cargo clean --manifest-path %s/Cargo.toml --target-dir %s\n' "$mj_rc_crate" "$mj_rc_real"
    } >&2
    return 12
  fi

  RUSTFLAGS='' cargo clean --manifest-path "$mj_rc_crate/Cargo.toml" --target-dir "$mj_rc_target"
}

# mj_rust_bin_missing <repository-root> <path> [<prefix>]
#
# What to say when the executable a caller was pointed at is not there. Every caller that
# resolves one says it through this function, because the sentence is the expensive part: a
# missing executable read as a failure of the code under test costs a debugging session and a
# retracted result, and read as what it is costs a rebuild. On 2026-09-10 it was read the
# first way twice, and a branch was believed broken both times.
#
# This is the sibling of the generation guard in `majordomus generate`, which refuses an
# executable built from another revision of the crate (exit 15) and says `the tree is not
# stale; this executable is, so nothing was written and nothing was judged`. Same shape, one
# state earlier: there the executable is the wrong one, here there is none. Both say where
# the fault is not, because that is the inference the reader gets wrong.
#
# The prefix is the caller's own (`majordomus-cli: `, or a test harness's four spaces); the
# first line keeps the words `is not an executable`, which callers of the launcher match on.
mj_rust_bin_missing() {
  mj_bm_root="$1"
  mj_bm_path="$2"
  mj_bm_p="${3:-}"
  mj_bm_outside=0
  mj_rust_outside "$mj_bm_root" "$mj_bm_path" && mj_bm_outside=1
  {
    printf '%s%s is not an executable\n' "$mj_bm_p" "$mj_bm_path"
    printf '%s  The tree is not broken; the executable it was pointed at is gone. Nothing ran,\n' "$mj_bm_p"
    printf '%s  so nothing was measured and nothing was judged: this is a fact about this\n' "$mj_bm_p"
    printf '%s  environment, not a result about the code.\n' "$mj_bm_p"
    if [ -n "${MAJORDOMUS_BIN:-}" ]; then
      printf '%s  MAJORDOMUS_BIN names it; whoever set that variable owns whether it still exists.\n' "$mj_bm_p"
    fi
    if [ "$mj_bm_outside" = 1 ]; then
      printf '%s  It is outside this checkout (%s):\n' "$mj_bm_p" "$mj_bm_root"
      printf '%s  a build directory this one does not own, which its owner may reclaim at any moment.\n' "$mj_bm_p"
    fi
    printf '%s  Build your own: cargo build --manifest-path %s/apps/majordomus-cli/Cargo.toml\n' "$mj_bm_p" "$mj_bm_root"
  } >&2
}

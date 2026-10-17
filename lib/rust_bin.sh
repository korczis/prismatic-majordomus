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
# The root mj_rust_bin and mj_rust_stale take is the TOOL's — the distribution the
# executable ships in — and not the repository the tool is working on. Inside the shell tool
# that is $MJ_HOME, never $MJ_ROOT (lib/common.sh keeps the two apart). In this source
# checkout the two are one directory, which is why passing $MJ_ROOT looked right: in a
# packaged install (~/.local/share/majordomus/versions/<v>/{bin,lib,libexec,share}) serving
# another repository, $MJ_ROOT has no libexec/ and no crate, so the executable was never
# found and every Rust-backed feature of the shell tool degraded in silence (0.10.0). The
# repository reaches the executable as `--repo "$MJ_ROOT"`; it never decides which
# executable runs. test/cases/744_an_installed_tool_finds_its_own_executable.sh proves it
# from a packaged layout, and refuses a lib/ call that passes $MJ_ROOT here again.
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

# mj_rust_bin <tool-root>
#
# The tool's root, as the header says: $MJ_HOME inside the shell tool, the launcher's own
# directory in bin/majordomus-cli and bin/majordomus-env.
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

# mj_rust_stale <tool-root> <executable>
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

# ------------------------------------------------------------------------------ the floor
#
# How much free space a build of this crate must find before it starts.
#
# This repository already bounds every wait (project.every-wait-is-bounded): a wait without a
# timeout is a hang with a nicer name. A build with no bound on the disk it may consume is
# the same shape, and on 2026-09-12 it ended the same way — the volume reached zero bytes
# free while several sessions were building at once, and every one of them died at a moment
# when nobody could run `df` any more. What fails in that state is not the build: it is the
# harness writing its own task files, which reads like tooling breaking rather than a full
# disk. A diagnostic that arrives afterwards helps the next person; a bound that refuses to
# start is what helps the person about to be stuck.
#
#   MAJORDOMUS_MIN_FREE_MB   free MiB below which a build refuses to start (default 5120);
#                            0 disables the bound entirely.
#
# The default is measured, not chosen. On 2026-09-12 a full `cargo build --locked` of
# apps/majordomus-cli into an empty target/, with a warm sccache, wrote 2,566,652 KiB —
# 2.45GiB. The floor is two of those: a build that passes this test has the room to finish,
# and the tree it finishes in still has room for the release and test artifacts the same
# session adds on top. The build directories on this machine that day held between 2.4GB and
# 23.4GB each, median 3.4GB, which is what a worktree costs over its working life.
MJ_MIN_FREE_MB_DEFAULT=5120

# mj_free_mb <path>
#
# MiB free on the volume that path is on, or nothing at all when it cannot be read. The path
# need not exist yet — a build directory usually does not — so the nearest existing ancestor
# is what is measured. Nothing, never a number that was not measured: a bound that invents
# its input is the defect project.destructive-sweeps-fail-closed names, one domain over.
mj_free_mb() {
  mj_fm_path="$1"
  while [ -n "$mj_fm_path" ] && [ "$mj_fm_path" != "/" ] && [ ! -d "$mj_fm_path" ]; do
    mj_fm_path="$(dirname "$mj_fm_path")"
  done
  [ -d "$mj_fm_path" ] || return 1
  df -k "$mj_fm_path" 2>/dev/null | awk 'NR == 2 && $4 ~ /^[0-9]+$/ { printf "%d", $4/1024; exit }'
}

# mj_rust_space_check <repository-root> [what]
#
# May a build start here? True (0) to proceed, false (1) to refuse — and it says why on
# stderr, in the words that make the next move obvious.
#
# The asymmetry with a deleting sweep is deliberate. A sweep whose predicate cannot be
# measured must treat that as "do not delete"; a bound whose input cannot be measured must
# not treat it as "nobody may build", because refusing every build on a machine whose `df`
# is unreadable stops all work to prevent a hypothetical. So: unreadable says so out loud
# and lets the build run; measured-and-below refuses.
mj_rust_space_check() {
  mj_sc_root="$1"
  mj_sc_what="${2:-a build}"
  # what this build's own build directory holds idle goes first, so that the floor below
  # judges the room that is really left (I2713)
  mj_rust_prune_incremental "$mj_sc_root"
  mj_sc_floor="${MAJORDOMUS_MIN_FREE_MB:-$MJ_MIN_FREE_MB_DEFAULT}"
  case "$mj_sc_floor" in ''|*[!0-9]*) mj_sc_floor="$MJ_MIN_FREE_MB_DEFAULT" ;; esac
  [ "$mj_sc_floor" -gt 0 ] || return 0

  # the volume the build writes to, which is not necessarily the one the checkout is on
  mj_sc_target="$(mj_cargo_target_dir "$mj_sc_root/apps/majordomus-cli")"
  mj_sc_free="$(mj_free_mb "$mj_sc_target" || true)"
  if [ -z "$mj_sc_free" ]; then
    printf 'majordomus: free space on %s could not be measured; %s is starting anyway\n' \
      "$mj_sc_target" "$mj_sc_what" >&2
    return 0
  fi
  [ "$mj_sc_free" -lt "$mj_sc_floor" ] || return 0

  {
    printf 'majordomus: refusing to start %s: %sMB free, floor %sMB.\n' \
      "$mj_sc_what" "$mj_sc_free" "$mj_sc_floor"
    printf '  Where it would write:  %s\n' "$mj_sc_target"
    printf '  What one build costs:  2.45GB into target/ (measured 2026-09-12: empty target, warm sccache)\n'
    printf '  A build started below this floor does not fail by itself. It fills the volume, and every\n'
    printf '  other session on this machine dies at a point where nothing can write a file any more,\n'
    printf '  which reads to each of them as broken tooling rather than as a full disk.\n'
    printf '  Largest build directories (after this build'"'"'s idle incremental sessions were pruned):\n'
    mj_rust_largest_build_dirs "$mj_sc_root" | sed 's/^/    /'
    printf '  Reclaim first:         scripts/reap-orphans --targets    (what is reclaimable; changes nothing)\n'
    printf '                         scripts/reap-orphans --reclaim    (build output of landed, unattended worktrees)\n'
    printf '  To override:           MAJORDOMUS_MIN_FREE_MB=0\n'
  } >&2
  return 1
}

# ------------------------------------------------------------------------------ idle incremental sessions
#
# What a build directory's incremental cache keeps, and for how long (I2713).
#
# rustc keeps a crate's incremental state as session directories,
# <build-dir>/<profile>/incremental/<crate>-<hash>/s-*: the one being written ends in
# `-working`, a finished one is what the next compile of that crate copies from. Only a later
# compile of the same crate removes a finished one, so every crate that stops being compiled
# — a test target renamed, a dependency bumped, a worktree landed — leaves its last session
# behind for good. In a build directory every worktree shares (ADR 0139) that is every
# worktree's leftovers. On 2026-10-14 one build directory held 24 GiB of incremental state in
# 574 sessions, one of them live, while free disk fell from 71 GiB to 4 GiB and every
# session's derive refused; on 2026-10-16 the shared one held 79 GB in 454 sessions, five of
# them live, with 6.9 GiB free at a load of 55. Removing the idle ones by hand gave back
# 16 GiB and 57 GiB, with compiles running both times.
#
#   MAJORDOMUS_INCREMENTAL_IDLE_MIN  minutes a finished session must have gone unwritten
#                                    before a build removes it (default below); 0 removes none.
#
# The default is a judgement, said as one: an edit-compile loop rebuilds well inside half an
# hour, so a session younger than that is likely the one its worktree's next compile copies
# from, and one older costs at most one recompile of its crate if it is ever wanted again.
# Both hand sweeps above took everything idle for five minutes and broke no build.
MJ_INCREMENTAL_IDLE_MIN_DEFAULT=30

# mj_cargo_metadata_dir <crate-directory> <build_directory|target_directory>
#
# That directory as cargo itself reports it. `build_directory` folds in CARGO_BUILD_BUILD_DIR,
# a build.build-dir of any .cargo/config.toml and the {cargo-cache-home} it expands to, which
# is not the shell's CARGO_HOME when an asdf shim sets that variable for cargo alone (it is
# how the shared directory of this machine came to live under ~/.asdf/installs/rust/). Prints
# nothing and fails when cargo cannot be asked. `command cargo`, because scripts/rust-check
# defines a `cargo` function that builds through mj_cargo_bounded.
#
# Asked from inside the crate, because rustup picks the toolchain by the working directory
# and not by --manifest-path: from anywhere else the machine's default toolchain answers,
# and a cargo older than build-dir reports no `build_directory` at all (on 2026-10-16 the
# default here was 1.90.0, the pin 1.98.1).
mj_cargo_metadata_dir() {
  command -v cargo >/dev/null 2>&1 || return 1
  mj_cmd_json="$(cd "$1" 2>/dev/null && command cargo metadata --format-version 1 --no-deps \
    --manifest-path "$1/Cargo.toml" 2>/dev/null)" || return 1
  mj_cmd_dir="$(printf '%s' "$mj_cmd_json" | sed -n "s/.*\"$2\":\"\\([^\"]*\\)\".*/\\1/p")"
  case "$mj_cmd_dir" in /*) printf '%s\n' "$mj_cmd_dir" ;; *) return 1 ;; esac
}

# mj_cargo_build_dir <crate-directory>
#
# Where cargo keeps that crate's intermediates, the incremental state among them. A cargo
# older than build-dir reports no `build_directory` and keeps them in the target directory,
# which is then the answer. Never a guessed path: what this answers is where a sweep deletes.
mj_cargo_build_dir() {
  mj_cargo_metadata_dir "$1" build_directory || mj_cargo_metadata_dir "$1" target_directory
}

# mj_rust_is_build_dir <path> — does it carry a mark only cargo leaves? The marks
# scripts/reap-orphans asks for before it removes a build directory.
mj_rust_is_build_dir() {
  [ "$(head -n 1 "$1/CACHEDIR.TAG" 2>/dev/null || true)" = 'Signature: 8a477f597d28d172789f06886806bc55' ] \
    && return 0
  [ -f "$1/.rustc_info.json" ] && return 0
  [ -d "$1/debug/.fingerprint" ] && return 0
  [ -d "$1/release/.fingerprint" ] && return 0
  return 1
}

# mj_rust_mtime <path> — its modification time in seconds since the epoch, or nothing and a
# failure. GNU's spelling first: BSD stat refuses -c, while GNU stat reads BSD's -f as "the
# file system" and prints something that is not a time.
mj_rust_mtime() {
  mj_mt="$(stat -c %Y "$1" 2>/dev/null || stat -f %m "$1" 2>/dev/null || true)"
  case "$mj_mt" in ''|*[!0-9]*) return 1 ;; esac
  printf '%s\n' "$mj_mt"
}

# mj_rust_kb <path> — its size in KiB as du counts it, or nothing and a failure.
mj_rust_kb() {
  mj_kb="$(du -sk "$1" 2>/dev/null | awk 'NR == 1 && $1 ~ /^[0-9]+$/ { print $1 }')"
  [ -n "$mj_kb" ] || return 1
  printf '%s\n' "$mj_kb"
}

# mj_rust_human_kb <KiB> — 1.2GB or 340MB.
mj_rust_human_kb() {
  awk -v k="$1" 'BEGIN { if (k >= 1048576) printf "%.1fGB", k / 1048576; else printf "%dMB", k / 1024 }'
}

# mj_rust_prune_incremental <tree-root>
#
# Remove the idle incremental sessions of the build directory this tree's build is about to
# use, and say what was reclaimed. A session is idle when its name says it is finished (no
# `-working`) and nothing in it was written for MAJORDOMUS_INCREMENTAL_IDLE_MIN minutes.
# Nothing else is ever removed: not a live session, not the lock files beside the sessions
# (rustc collects those), not another build directory, not a target directory.
#
# A deleting sweep, so it fails closed (project.destructive-sweeps-fail-closed):
#   - the build directory is cargo's answer or nothing, and unanswered removes nothing;
#   - it must carry cargo's mark, and an incremental directory must physically be inside it;
#   - a listing that could not be read whole removes nothing at all. The one error that is
#     not a hole is an entry that vanished while it was read: rustc removes a session it
#     has superseded, and what is gone is no longer a candidate;
#   - each candidate's idle minutes and size are measured and printed before anything is
#     removed, and one whose age or size could not be read is skipped and counted;
#   - the moment before each removal the predicate is asked again, of that path alone.
#
# Always returns 0: a sweep that could not run says so, and the build goes on to the floor.
# Once per tree and build directory in a shell, so that a caller that asks the floor and then
# builds through mj_cargo_bounded sweeps once.
mj_rust_prune_incremental() {
  mj_pi_root="$1"
  mj_pi_age="${MAJORDOMUS_INCREMENTAL_IDLE_MIN:-$MJ_INCREMENTAL_IDLE_MIN_DEFAULT}"
  case "$mj_pi_age" in ''|*[!0-9]*) mj_pi_age="$MJ_INCREMENTAL_IDLE_MIN_DEFAULT" ;; esac
  [ "$mj_pi_age" -gt 0 ] || return 0
  mj_pi_key="$mj_pi_root|${CARGO_BUILD_BUILD_DIR:-}|${CARGO_TARGET_DIR:-}"
  [ "${MJ_INCREMENTAL_PRUNED_FOR:-}" = "$mj_pi_key" ] && return 0
  MJ_INCREMENTAL_PRUNED_FOR="$mj_pi_key"

  mj_pi_say='majordomus: idle incremental sessions'
  mj_pi_dir="$(mj_cargo_build_dir "$mj_pi_root/apps/majordomus-cli" || true)"
  if [ -z "$mj_pi_dir" ]; then
    printf '%s: none removed: cargo did not say where this build keeps them (cargo metadata failed)\n' \
      "$mj_pi_say" >&2
    return 0
  fi
  [ -d "$mj_pi_dir" ] || return 0   # nothing was ever built there, so there is nothing to reclaim
  if ! mj_rust_is_build_dir "$mj_pi_dir"; then
    printf '%s: none removed: %s carries no mark of cargo'"'"'s, so it is not provably a build directory\n' \
      "$mj_pi_say" "$mj_pi_dir" >&2
    return 0
  fi
  mj_pi_find='find'
  [ -x /usr/bin/find ] && mj_pi_find=/usr/bin/find
  mj_pi_list="$(mktemp "${TMPDIR:-/tmp}/mj-incremental.XXXXXX")" || return 0
  mj_pi_err="$mj_pi_list.err"
  mj_pi_plan="$mj_pi_list.plan"
  : > "$mj_pi_plan"

  # 1. the listing: every profile's incremental directory, whole or not at all
  for mj_pi_inc in "$mj_pi_dir"/*/incremental "$mj_pi_dir"/*/*/incremental; do
    [ -d "$mj_pi_inc" ] && [ -d "$(dirname "$mj_pi_inc")/.fingerprint" ] || continue
    mj_rust_owns "$mj_pi_dir" "$mj_pi_inc" || continue
    if ! "$mj_pi_find" "$mj_pi_inc" -mindepth 2 -maxdepth 2 -type d -name 's-*' ! -name '*-working' \
        -mmin +"$mj_pi_age" >> "$mj_pi_list" 2>"$mj_pi_err"; then
      if [ ! -s "$mj_pi_err" ] || grep -v 'No such file or directory' "$mj_pi_err" | grep -q .; then
        printf '%s: none removed: %s could not be read whole: %s\n' "$mj_pi_say" "$mj_pi_inc" \
          "$(head -n 1 "$mj_pi_err" 2>/dev/null || true)" >&2
        rm -f "$mj_pi_list" "$mj_pi_err" "$mj_pi_plan"
        return 0
      fi
    fi
  done

  # 2. the measurement of each candidate, kept beside it
  mj_pi_now="$(date +%s)"
  mj_pi_n=0; mj_pi_skipped=0
  while IFS= read -r mj_pi_s; do
    [ -n "$mj_pi_s" ] || continue
    mj_pi_mt="$(mj_rust_mtime "$mj_pi_s" || true)"
    mj_pi_kb="$(mj_rust_kb "$mj_pi_s" || true)"
    if [ -z "$mj_pi_mt" ] || [ -z "$mj_pi_kb" ]; then
      mj_pi_skipped=$((mj_pi_skipped + 1)); continue
    fi
    mj_pi_idle=$(( (mj_pi_now - mj_pi_mt) / 60 ))
    [ "$mj_pi_idle" -ge "$mj_pi_age" ] || continue   # written again since it was listed
    printf '%s %s %s\n' "$mj_pi_kb" "$mj_pi_idle" "$mj_pi_s" >> "$mj_pi_plan"
    mj_pi_n=$((mj_pi_n + 1))
  done < "$mj_pi_list"

  # 3. said, then asked again, then removed
  if [ "$mj_pi_n" -gt 0 ]; then
    mj_pi_before="$(mj_free_mb "$mj_pi_dir" || true)"
    {
      printf '%s of %s, unwritten for over %s min: removing %s\n' \
        "$mj_pi_say" "$mj_pi_dir" "$mj_pi_age" "$mj_pi_n"
      while read -r mj_pi_kb mj_pi_idle mj_pi_s; do
        printf '  %8s  idle %5s min  %s\n' "$(mj_rust_human_kb "$mj_pi_kb")" "$mj_pi_idle" \
          "${mj_pi_s#"$mj_pi_dir"/}"
      done < "$mj_pi_plan"
    } >&2
    mj_pi_removed=0; mj_pi_sum=0; mj_pi_changed=0; mj_pi_failed=0
    while read -r mj_pi_kb mj_pi_idle mj_pi_s; do
      if [ "$("$mj_pi_find" "$mj_pi_s" -maxdepth 0 -type d -name 's-*' ! -name '*-working' \
            -mmin +"$mj_pi_age" 2>/dev/null)" != "$mj_pi_s" ]; then
        mj_pi_changed=$((mj_pi_changed + 1)); continue
      fi
      if rm -rf "$mj_pi_s" 2>/dev/null && [ ! -e "$mj_pi_s" ]; then
        mj_pi_removed=$((mj_pi_removed + 1)); mj_pi_sum=$((mj_pi_sum + mj_pi_kb))
      else
        mj_pi_failed=$((mj_pi_failed + 1))
      fi
    done < "$mj_pi_plan"
    mj_pi_after="$(mj_free_mb "$mj_pi_dir" || true)"
    # du's sizes, said as such: a file rustc hard-linked into a live session stays on disk,
    # so the volume's own before and after is the number that was really given back
    printf '%s: removed %s holding %s; free on its volume %sMB before, %sMB after\n' \
      "$mj_pi_say" "$mj_pi_removed" "$(mj_rust_human_kb "$mj_pi_sum")" \
      "${mj_pi_before:-?}" "${mj_pi_after:-?}" >&2
    [ "$mj_pi_changed" = 0 ] || printf '%s: kept %s written to or renamed since it was measured\n' \
      "$mj_pi_say" "$mj_pi_changed" >&2
    [ "$mj_pi_failed" = 0 ] || printf '%s: %s could not be removed\n' "$mj_pi_say" "$mj_pi_failed" >&2
  fi
  [ "$mj_pi_skipped" = 0 ] || printf '%s: skipped %s whose age or size could not be read\n' \
    "$mj_pi_say" "$mj_pi_skipped" >&2
  rm -f "$mj_pi_list" "$mj_pi_err" "$mj_pi_plan"
  return 0
}

# mj_rust_largest_build_dirs <tree-root>
#
# Where this machine's build output is, largest first, as `<size>  <path>` lines, at most
# five: this build's own build directory and every cargo-marked directory beside it, each
# tree's private one (mj_cargo_private_dir), and this tree's target directory. A size that
# could not be measured is `?`, never a guess. The refusal path's alone: it costs a du each.
mj_rust_largest_build_dirs() {
  mj_lb_crate="$1/apps/majordomus-cli"
  mj_lb_build="$(mj_cargo_build_dir "$mj_lb_crate" || true)"
  mj_lb_target="$(mj_cargo_metadata_dir "$mj_lb_crate" target_directory || mj_cargo_target_dir "$mj_lb_crate")"
  {
    if [ -n "$mj_lb_build" ]; then
      printf '%s\n' "$mj_lb_build"
      for mj_lb_d in "$(dirname "$mj_lb_build")"/*; do
        mj_rust_is_build_dir "$mj_lb_d" && printf '%s\n' "$mj_lb_d"
      done
    fi
    for mj_lb_d in "${CARGO_HOME:-$HOME/.cargo}"/build-private/*; do printf '%s\n' "$mj_lb_d"; done
    printf '%s\n' "$mj_lb_target"
  } | while IFS= read -r mj_lb_d; do
    mj_rust_physical "$mj_lb_d" || true
  done | sort -u | while IFS= read -r mj_lb_d; do
    printf '%s\t%s\n' "$(mj_rust_kb "$mj_lb_d" || printf '%s' -1)" "$mj_lb_d"
  done | sort -rn | head -n 5 | while IFS="$(printf '\t')" read -r mj_lb_kb mj_lb_d; do
    if [ "$mj_lb_kb" = -1 ]; then mj_lb_h='?'; else mj_lb_h="$(mj_rust_human_kb "$mj_lb_kb")"; fi
    printf '%8s  %s\n' "$mj_lb_h" "$mj_lb_d"
  done
  return 0
}

# How long a build may wait for another build's lock before it stops waiting, in seconds.
#
#   MAJORDOMUS_BUILD_LOCK_WAIT  seconds a build may stand behind another build's lock on the
#                               build directory before it builds in its own (default below);
#                               0 never waits: it builds in its own at the first sign.
#
# Measured, not chosen: on 2026-10-13 a commit hook's `cargo build --quiet` stood behind the
# lock of the shared build directory (ADR 0139) for 14 minutes at 0% CPU with no child and
# no line of output, while nineteen cargo processes of other worktrees took their turns. A
# build of this crate that finds its dependencies compiled takes under a minute, so a minute
# behind a lock is already longer than the work.
MJ_BUILD_LOCK_WAIT_DEFAULT=60

# mj_cargo_private_dir <tree-root>
#
# The build directory that is this tree's alone, under cargo's home like the shared one, so
# that it is one path whatever the tree's depth. Named by the tree's root, which is what
# makes two worktrees two directories.
mj_cargo_private_dir() {
  printf '%s/build-private/%s\n' "${CARGO_HOME:-$HOME/.cargo}" \
    "$(printf '%s' "$1" | cksum | awk '{ print $1 }')"
}

# mj_cargo_bounded <tree-root> <cargo arguments...>
#
# Run cargo, and never wait in silence behind another build's lock
# (project.a-build-never-waits-in-silence-on-anothers-lock, ADR 0139's amendment).
#
# The worktrees of this repository share one build directory, and cargo takes one exclusive
# lock on it, so two builds at once are one build and one wait. Cargo says so — "Blocking
# waiting for file lock on build directory" — but only as a status line, which `--quiet`
# removes, and every caller here passed `--quiet`. So the wait had no bound, no message and
# no way out, which is the three things project.every-wait-is-bounded asks of a wait.
#
# What this does instead:
#   - it runs cargo without `--quiet` and keeps cargo's stderr in a file, so the status line
#     exists to be read; on success nothing of it is printed (the caller asked for quiet), on
#     failure all of it is;
#   - while the last thing cargo said is that it is blocked, it counts; at the bound it says
#     who else is building, stops that cargo, and runs the same build in this tree's own
#     build directory (mj_cargo_private_dir), after the disk floor has been asked;
#   - a build that is already in a directory of its own (CARGO_BUILD_BUILD_DIR set by the
#     caller) and is still blocked at the bound has no second place to go: it stops with
#     exit 75 and says what holds it, rather than wait for ever.
#
# Exit status: cargo's own; 12 when the disk floor refuses the private build; 75 when the
# bound was reached with nowhere left to build.
mj_cargo_bounded() {
  mj_cb_root="$1"; shift
  mj_cb_wait="${MAJORDOMUS_BUILD_LOCK_WAIT:-$MJ_BUILD_LOCK_WAIT_DEFAULT}"
  case "$mj_cb_wait" in ''|*[!0-9]*) mj_cb_wait="$MJ_BUILD_LOCK_WAIT_DEFAULT" ;; esac
  # the caller's --quiet is honoured by this function, not by cargo: see above
  mj_cb_quiet=0
  for mj_cb_arg in "$@"; do
    shift
    case "$mj_cb_arg" in --quiet|-q) mj_cb_quiet=1 ;; *) set -- "$@" "$mj_cb_arg" ;; esac
  done
  # a build never starts on top of its build directory's idle incremental sessions (I2713);
  # a caller that asked the floor first has swept already, and this is then a no-op
  mj_rust_prune_incremental "$mj_cb_root"
  mj_cb_log="$(mktemp "${TMPDIR:-/tmp}/mj-cargo.XXXXXX")" || return 12
  mj_cb_private=0
  [ -n "${CARGO_BUILD_BUILD_DIR:-}" ] && mj_cb_private=1
  while :; do
    : > "$mj_cb_log"
    command cargo "$@" 2>"$mj_cb_log" &
    mj_cb_pid=$!
    # a caller that did not ask for quiet sees cargo's lines as they are written, not at the
    # end: a test run of ten minutes that says nothing reads as a hang
    mj_cb_tail=""
    if [ "$mj_cb_quiet" = 0 ]; then
      tail -f "$mj_cb_log" >&2 2>/dev/null &
      mj_cb_tail=$!
    fi
    mj_cb_blocked=0
    mj_cb_gave_up=0
    while kill -0 "$mj_cb_pid" 2>/dev/null; do
      if tail -n 1 "$mj_cb_log" 2>/dev/null | grep -q 'waiting for file lock on'; then
        if [ "$mj_cb_blocked" -ge "$mj_cb_wait" ]; then mj_cb_gave_up=1; break; fi
        mj_cb_blocked=$((mj_cb_blocked + 1))
      else
        mj_cb_blocked=0
      fi
      sleep 1
    done
    if [ "$mj_cb_gave_up" = 0 ]; then
      mj_cb_rc=0
      wait "$mj_cb_pid" || mj_cb_rc=$?
      if [ -n "$mj_cb_tail" ]; then
        sleep 1; kill "$mj_cb_tail" 2>/dev/null || true; wait "$mj_cb_tail" 2>/dev/null || true
      elif [ "$mj_cb_rc" != 0 ]; then
        cat "$mj_cb_log" >&2
      fi
      rm -f "$mj_cb_log"
      return "$mj_cb_rc"
    fi
    # the bound: say what was observed, stop waiting, and go where no other tree builds
    kill "$mj_cb_pid" 2>/dev/null || true
    wait "$mj_cb_pid" 2>/dev/null || true
    if [ -n "$mj_cb_tail" ]; then kill "$mj_cb_tail" 2>/dev/null || true; wait "$mj_cb_tail" 2>/dev/null || true; fi
    mj_cb_others="$(pgrep -x cargo 2>/dev/null | grep -v "^$mj_cb_pid\$" | head -n 5 | tr '\n' ' ' || true)"
    printf 'majordomus: this build waited %ss behind another build'"'"'s lock on its build directory (cargo: "Blocking waiting for file lock"); other cargo processes: %s\n' \
      "$mj_cb_blocked" "${mj_cb_others:-none found}" >&2
    if [ "$mj_cb_private" = 1 ]; then
      printf 'majordomus: the build directory is already this tree'"'"'s own (%s), so there is nowhere else to build; stopping instead of waiting. Find the holder: pgrep -lf cargo\n' \
        "${CARGO_BUILD_BUILD_DIR:-unset}" >&2
      rm -f "$mj_cb_log"
      return 75
    fi
    mj_rust_space_check "$mj_cb_root" "a build in this tree's own build directory" || { rm -f "$mj_cb_log"; return 12; }
    CARGO_BUILD_BUILD_DIR="$(mj_cargo_private_dir "$mj_cb_root")"
    export CARGO_BUILD_BUILD_DIR
    mj_cb_private=1
    printf 'majordomus: building in this tree'"'"'s own build directory instead: %s (a cold one compiles every dependency once)\n' \
      "$CARGO_BUILD_BUILD_DIR" >&2
  done
}

# The reasoning report of the open task (ADR 0098), as `majordomus reasoning status
# --report` renders it: the decisions with who reviewed them, what is unresolved, and the
# degradation on the way. One renderer, the executable's, so that `context`, a derived
# handover and every other surface say the same thing. Never a build, as for the peer
# board: an executable that is not there yields nothing, and so does a task with no
# reasoning records. Prints the report or nothing; always returns 0.
#
# mj_rust_reasoning_report <repository-root> [<tool-root>]
#
# Two roots, because the records are the repository's and the executable is the tool's.
# The tool root defaults to the repository's only for a caller in this source checkout,
# where they coincide; the shell tool passes "$MJ_ROOT" "$MJ_HOME".
mj_rust_reasoning_report() {
  mj_rr_root="$1"
  mj_rr_bin="$(mj_rust_bin "${2:-$1}")"
  [ -x "$mj_rr_bin" ] || return 0
  [ -d "$mj_rr_root/.ai/local/state/reasoning" ] || return 0
  mj_rr_share="$(mj_rust_share "$mj_rr_root")"
  ( [ -z "$mj_rr_share" ] || export MAJORDOMUS_SHARE="$mj_rr_share"
    "$mj_rr_bin" reasoning status --report --repo "$mj_rr_root" ) 2>/dev/null || true
  return 0
}

# The pull-request integration queue in one line, as `majordomus prs brief` renders it from
# what this checkout last recorded: the queue summary, the lease and how far it reaches, the
# last merge and the last outcome. It decides no relation and reaches no network, and it
# degrades to nothing: no executable, or a checkout whose forge was never observed. One
# renderer, the executable's, so that `context` and a derived handover say the same line.
# Never a build, as for the peer board. Prints the line or nothing; always returns 0.
#
# mj_rust_prs_brief <repository-root> [<tool-root>]
# mj_rust_monitor_brief <root> [home] — what happened around this repository in the last
# hour, for a briefing (ADR 0133): one summary line, then at most five signals that name the
# reader ($MAJORDOMUS_MONITOR_FOR) and five that ask for attention, newest first. Local
# sources only (the mesh and the executor's trail): the start of a session is no place to
# wait on the network. Prints nothing when there is no executable, no server or no jq — a
# briefing degrades to silence, never to an error.
mj_rust_monitor_brief() {
  mj_mb_root="$1"
  command -v jq >/dev/null 2>&1 || return 0
  mj_mb_bin="$(mj_rust_bin "${2:-$1}")"
  [ -x "$mj_mb_bin" ] || return 0
  mj_mb_share="$(mj_rust_share "${2:-$1}")"
  mj_mb_out="$( ( [ -z "$mj_mb_share" ] || export MAJORDOMUS_SHARE="$mj_mb_share"
    "$mj_mb_bin" monitor --repo "$mj_mb_root" --since 1h --limit 200 --sources mesh,executor --format json ) 2>/dev/null)" || return 0
  printf '%s' "$mj_mb_out" | jq -r '
    select(.signals != null)
    | "\(.signals | length) signal(s) in the last hour, \(.for_you) for you, \(.suppressed) folded away as noise — majordomus monitor --follow streams it",
      ([.signals[] | select(.for_you)] | reverse | .[0:5][] | "  for you    " + .summary),
      ([.signals[] | select((.for_you | not) and .severity != "info")] | reverse | .[0:5][] | "  " + .severity + "  " + .summary)
  ' 2>/dev/null || true
  return 0
}

mj_rust_prs_brief() {
  mj_pb_root="$1"
  mj_pb_bin="$(mj_rust_bin "${2:-$1}")"
  [ -x "$mj_pb_bin" ] || return 0
  mj_pb_share="$(mj_rust_share "${2:-$1}")"
  ( [ -z "$mj_pb_share" ] || export MAJORDOMUS_SHARE="$mj_pb_share"
    "$mj_pb_bin" prs brief --repo "$mj_pb_root" ) 2>/dev/null || true
  return 0
}

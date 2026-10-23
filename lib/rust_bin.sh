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

# mj_rust_installed <repository-root>
#
# The executable of the installed release, for the caller that is a client waiting on a
# handshake and cannot wait on cargo: the one `bin/majordomus-cli mcp` falls back to when this
# checkout has none to run (ADR 0172). Empty and false when there is none.
#
#   MAJORDOMUS_INSTALLED_BIN  the installed release's launcher (default: majordomus in
#                             $MJ_INSTALL_DIR, which is ~/.local/bin, where install.sh puts it)
#
# Never a file inside the repository asked about: a launcher that found itself would run
# itself, and a client would wait on a loop. It must also answer --version, because a file
# that is merely executable (a dangling wrapper, a half-swapped install) is the same
# silence this function exists to end.
mj_rust_installed() {
  mj_ri_root="$1"
  mj_ri_bin="${MAJORDOMUS_INSTALLED_BIN:-${MJ_INSTALL_DIR:-$HOME/.local/bin}/majordomus}"
  [ -x "$mj_ri_bin" ] || return 1
  mj_ri_real="$(cd "$(dirname "$mj_ri_bin")" 2>/dev/null && pwd -P)/$(basename "$mj_ri_bin")"
  case "$mj_ri_real" in "$(cd "$mj_ri_root" 2>/dev/null && pwd -P)/"*) return 1 ;; esac
  "$mj_ri_bin" --version >/dev/null 2>&1 || return 1
  printf '%s\n' "$mj_ri_bin"
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

#   MAJORDOMUS_RECLAIM_BELOW_MB   free MiB below which a build first runs the reaper's
#                                 classified reclaim (default 51200); 0 disables it.
#
# The floor alone was a cliff edge, not a bound. On 2026-10-10 two machines reached 99% (12GB
# and 7GB free) with 270GB and 48GB of build output in worktrees nobody was working in, and
# nothing ran the reaper: it is a command a person must remember, and the floor only refuses
# once there is nothing left to refuse with. So reclaiming moved to the demand side. Every
# build that starts with less than this free asks `scripts/reap-orphans --targets --reclaim`
# first — the classified sweep, safe unattended by construction (it removes build output of
# landed or pushed, clean worktrees nobody is in, and nothing else) — and only then applies the
# floor. 51200MiB is ten builds of the crate at their measured working size (the median build
# directory here is 3.4GB, the largest 25GB): far enough above the floor that the sweep runs
# while several sessions are still building, not after the last one has failed.
MJ_RECLAIM_BELOW_MB_DEFAULT=51200

# How often one machine runs that sweep at most, in seconds. The sweep reads `git status` of
# every worktree and takes about a minute on a machine with thirty of them; a build that waits
# a minute every time it starts under pressure would be its own outage. The stamp is a rate
# limit, never a deletion predicate: a stamp that cannot be read means "run the sweep".
MJ_RECLAIM_EVERY_S=600

# mj_rust_reclaim <repository-root> <free-mb>
#
# The demand-side half of project.accumulation-is-measured: below the high-water mark, run
# the reaper's reclaim once per interval, machine-wide, and say what it did on stderr. Never
# fails the build: a reaper that is missing or refuses leaves the floor to decide.
mj_rust_reclaim() {
  mj_rr_root="$1"; mj_rr_free="$2"
  mj_rr_mark="${MAJORDOMUS_RECLAIM_BELOW_MB:-$MJ_RECLAIM_BELOW_MB_DEFAULT}"
  case "$mj_rr_mark" in ''|*[!0-9]*) mj_rr_mark="$MJ_RECLAIM_BELOW_MB_DEFAULT" ;; esac
  [ "$mj_rr_mark" -gt 0 ] || return 0
  [ "$mj_rr_free" -lt "$mj_rr_mark" ] || return 0
  [ -x "$mj_rr_root/scripts/reap-orphans" ] || return 0
  mj_rr_stamp="${XDG_CACHE_HOME:-$HOME/.cache}/majordomus/reclaim.stamp"
  mj_rr_last="$(cat "$mj_rr_stamp" 2>/dev/null || true)"
  case "$mj_rr_last" in ''|*[!0-9]*) mj_rr_last=0 ;; esac
  mj_rr_now="$(date +%s)"
  [ $((mj_rr_now - mj_rr_last)) -ge "$MJ_RECLAIM_EVERY_S" ] || return 0
  mkdir -p "$(dirname "$mj_rr_stamp")" 2>/dev/null && printf '%s\n' "$mj_rr_now" > "$mj_rr_stamp" 2>/dev/null || true
  printf 'majordomus: %sMB free, below %sMB: reclaiming unattended build output first (scripts/reap-orphans --targets --reclaim)\n' \
    "$mj_rr_free" "$mj_rr_mark" >&2
  env -u MAJORDOMUS_SHARE "$mj_rr_root/scripts/reap-orphans" --targets --reclaim 2>&1 \
    | grep -E '^reap-orphans: ' | sed 's/^/  /' >&2 || true
  return 0
}

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
  mj_rust_prune_build_dir "$mj_sc_root"
  # and the private build directories builds fell back to and left (rate-limited)
  mj_rust_reclaim_private
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
  # below the high-water mark the classified sweep runs first, and the floor reads what is left
  mj_rust_reclaim "$mj_sc_root" "$mj_sc_free"
  mj_sc_free="$(mj_free_mb "$mj_sc_target" || printf '%s' "$mj_sc_free")"
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
    printf '                         scripts/reap-orphans --reclaim    (build output of landed or pushed, unattended worktrees)\n'
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

# mj_rust_prune_incremental_of <build-dir> <profile-dir> <age-minutes>
#
# Remove the idle incremental sessions of one profile of that build directory, and say what
# was reclaimed. A session is idle when its name says it is finished (no `-working`) and
# nothing in it was written for that many minutes. Nothing else is ever removed: not a live
# session, not the lock files beside the sessions (rustc collects those). Called under cargo's
# lock on the profile (mj_rust_locked), after mj_rust_profile_users found nobody using it.
#
# A deleting sweep, so it fails closed (project.destructive-sweeps-fail-closed):
#   - a listing that could not be read whole removes nothing at all. The one error that is
#     not a hole is an entry that vanished while it was read: rustc removes a session it
#     has superseded, and what is gone is no longer a candidate;
#   - each candidate's idle minutes and size are measured and printed before anything is
#     removed, and one whose age or size could not be read is skipped and counted;
#   - the moment before each removal the predicate is asked again, of that path alone.
mj_rust_prune_incremental_of() {
  mj_pi_dir="$1"; mj_pi_inc="$2/incremental"; mj_pi_age="$3"
  [ -d "$mj_pi_inc" ] || return 0
  mj_pi_say='majordomus: idle incremental sessions'
  mj_pi_find='find'
  [ -x /usr/bin/find ] && mj_pi_find=/usr/bin/find
  mj_pi_list="$(mktemp "${TMPDIR:-/tmp}/mj-incremental.XXXXXX")" || return 0
  mj_pi_err="$mj_pi_list.err"
  mj_pi_plan="$mj_pi_list.plan"
  : > "$mj_pi_plan"

  # 1. the listing of this profile's incremental directory, whole or not at all
  if ! "$mj_pi_find" "$mj_pi_inc" -mindepth 2 -maxdepth 2 -type d -name 's-*' ! -name '*-working' \
      -mmin +"$mj_pi_age" >> "$mj_pi_list" 2>"$mj_pi_err"; then
    if [ ! -s "$mj_pi_err" ] || grep -v 'No such file or directory' "$mj_pi_err" | grep -q .; then
      printf '%s: none removed: %s could not be read whole: %s\n' "$mj_pi_say" "$mj_pi_inc" \
        "$(head -n 1 "$mj_pi_err" 2>/dev/null || true)" >&2
      rm -f "$mj_pi_list" "$mj_pi_err" "$mj_pi_plan"
      return 0
    fi
  fi

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
# ------------------------------------------------------------------------------ stale crate artifacts
#
# What a build directory's deps directory keeps of this crate, and for how long (I2787).
#
# Every build of majordomus-cli writes its library, its executable, every test and bench
# binary and, on macOS, one object file per codegen unit into <profile>/deps, named
# <target>-<hash>: the hash is the unit's metadata, so a generation that stops being built —
# a dependency bumped, a feature or a profile setting changed, a test target renamed — leaves
# all of its files behind for good. In the build directory every worktree shares (ADR 0139)
# that is every worktree's leftovers, and nothing short of a `cargo clean` that wipes every
# live tree's build removes them. On 2026-10-16 the shared directory's deps reached 204 GB
# (84k files) with the disk at 29 GiB, and that night 41 GiB and 11026 majordomus_cli-* files
# remained after a hand sweep had removed 43 GiB; the next morning the volume reached zero
# and every session on the machine stopped.
#
#   MAJORDOMUS_DEPS_IDLE_MIN  minutes no file of a generation may have been written before a
#                             build removes that generation (default below); 0 removes none.
#
# The default is a judgement, said as one: the hand sweeps took generations idle for one and
# three hours and broke nothing that a rebuild did not mend, and cargo rebuilds a unit whose
# output is missing, so a generation removed too early costs one compile of this crate.
MJ_DEPS_IDLE_MIN_DEFAULT=120

# mj_rust_profile_dirs <build-dir>
#
# The profile directories of a build directory, one per line: debug and release, each target
# triple's, and those of a target directory cargo-llvm-cov nests inside it (llvm-cov-target/
# debug). A profile directory is one that holds cargo's .fingerprint, and it must physically
# be inside the build directory: a symlink out of it names somebody else's bytes.
mj_rust_profile_dirs() {
  for mj_pd_p in "$1"/* "$1"/*/*; do
    [ -d "$mj_pd_p/.fingerprint" ] || continue
    mj_rust_owns "$1" "$mj_pd_p" || continue
    printf '%s\n' "$mj_pd_p"
  done
}

# mj_rust_profile_users <profile-dir>
#
# The processes that name that profile directory in their command line, as `pid command`
# lines; nothing when none does. A test binary runs from <profile>/deps, rustdoc compiles a
# doctest with `-L dependency=<profile>/deps` and the crate's rlib from there, and rustc is
# handed its output directory there — and none of them holds cargo's lock: `cargo test`
# releases it when compiling ends and before the binaries and doctests it built run (measured
# 2026-10-17: a coverage run's test binaries were running with its profile's lock free). On
# 2026-10-16 a sweep that judged by age alone removed rlibs a running doctest was about to
# read. Fails (1) when the process table cannot be read, which a caller takes as "in use".
mj_rust_profile_users() {
  mj_pu_ps="$(ps -A -ww -o pid= -o args= 2>/dev/null)" || return 1
  [ -n "$mj_pu_ps" ] || return 1
  printf '%s\n' "$mj_pu_ps" | grep -F -- "$1/" | sed 's/^ *//' || true
}

# mj_rust_locked <profile-dir> <function> [arguments...]
#
# Run that function while holding cargo's own lock on the profile directory, the one a build
# holds exclusively from its first compile to its last (<profile>/.cargo-build-lock; a
# target directory of a cargo older than build-dir has <profile>/.cargo-lock). Taken without
# waiting: a profile some build holds is skipped and said, never swept and never waited on.
# The lock is flock(2), which cargo takes and flock(1) asks; without flock(1) nothing is
# swept, and that is said too. Returns 0 always: a sweep that did not run never fails a build.
mj_rust_locked() {
  mj_lk_p="$1"; shift
  mj_lk_file="$mj_lk_p/.cargo-build-lock"
  [ ! -f "$mj_lk_p/.cargo-lock" ] || [ -f "$mj_lk_file" ] || mj_lk_file="$mj_lk_p/.cargo-lock"
  if ! command -v flock >/dev/null 2>&1; then
    printf 'majordomus: %s not swept: flock(1) is not installed, so cargo'"'"'s lock on it cannot be taken\n' \
      "$mj_lk_p" >&2
    return 0
  fi
  # cargo creates the file when it first builds there and opens an existing one as it is
  [ -f "$mj_lk_file" ] || : >> "$mj_lk_file" 2>/dev/null || {
    printf 'majordomus: %s not swept: its lock file could not be opened\n' "$mj_lk_p" >&2
    return 0
  }
  mj_lk_rc=0
  ( flock -n -x 9 || exit 75; "$@" ) 9< "$mj_lk_file" || mj_lk_rc=$?
  if [ "$mj_lk_rc" = 75 ]; then
    printf 'majordomus: %s not swept: a build holds cargo'"'"'s lock on it\n' "$mj_lk_p" >&2
  fi
  return 0
}

# mj_rust_crate_stems <crate-directory>
#
# The file stems this crate's own units write into deps, one per line: its library (with and
# without `lib`), its executables, its tests and its benches, `-` read as cargo writes it.
# A stem that is also the name of a package in Cargo.lock is left out: a dependency's
# `<name>-<hash>.d` would match it, and another crate's artifact is never this sweep's.
mj_rust_crate_stems() {
  mj_cs_c="$1"
  mj_cs_lock="$(sed -n 's/^name = "\(.*\)"$/\1/p' "$mj_cs_c/Cargo.lock" 2>/dev/null \
    | grep -vx 'majordomus-cli' | tr '-' '_' || true)"
  {
    printf 'majordomus_cli\nlibmajordomus_cli\n'
    sed -n '/^\[\[bin\]\]/,/^name/s/^name = "\(.*\)"$/\1/p' "$mj_cs_c/Cargo.toml" 2>/dev/null
    for mj_cs_f in "$mj_cs_c"/tests/*.rs "$mj_cs_c"/benches/*.rs; do
      [ -f "$mj_cs_f" ] || continue
      mj_cs_b="${mj_cs_f##*/}"; printf '%s\n' "${mj_cs_b%.rs}"
    done
  } | tr '-' '_' | LC_ALL=C sort -u | while IFS= read -r mj_cs_s; do
    [ -n "$mj_cs_s" ] || continue
    printf '%s\n' "$mj_cs_lock" | grep -qxF -- "$mj_cs_s" || printf '%s\n' "$mj_cs_s"
  done
}

# mj_rust_generation <file-name> — the generation a deps file belongs to, `<stem>-<hash>`, or
# nothing when the name carries no 16-digit unit hash (then it is no generation of a unit and
# never a candidate). A library's `lib<stem>-<hash>.rlib` is the same generation as its
# `<stem>-<hash>.d` and object files, so the `lib` is not part of it.
mj_rust_generation() {
  printf '%s\n' "${1##*/}" | sed -n 's/^\(.*-[0-9a-f]\{16\}\)\(\..*\)\{0,1\}$/\1/p' | sed 's/^lib//'
}

# mj_rust_open_under <dir> — the paths under that directory some process holds open, one per
# line. Fails (1) when lsof is missing or could not read the process table, which a caller
# takes as "everything is open".
mj_rust_open_under() {
  command -v lsof >/dev/null 2>&1 || return 1
  mj_ou_err="$(mktemp "${TMPDIR:-/tmp}/mj-lsof.XXXXXX")" || return 1
  mj_ou_rc=0
  # lsof names what is open by its physical path, so it is asked of the physical directory
  mj_ou_dir="$(mj_rust_physical "$1")" || { rm -f "$mj_ou_err"; return 1; }
  mj_ou_out="$(lsof -n -P -F n +D "$mj_ou_dir" 2>"$mj_ou_err")" || mj_ou_rc=$?
  # lsof exits 1 when nothing is open; anything it says that is not a warning is a hole
  if [ "$mj_ou_rc" -gt 1 ] || grep -v 'WARNING' "$mj_ou_err" | grep -q .; then
    rm -f "$mj_ou_err"; return 1
  fi
  rm -f "$mj_ou_err"
  printf '%s\n' "$mj_ou_out" | sed -n 's/^n//p'
}

# mj_rust_prune_deps_of <profile-dir> <age-minutes> <stems-file>
#
# Remove the generations of this crate's units in <profile>/deps that no build has written for
# that many minutes and no process holds or names, and say what was reclaimed. Called under
# cargo's lock on that profile (mj_rust_locked), after mj_rust_profile_users found nobody.
# A deleting sweep, so it fails closed (project.destructive-sweeps-fail-closed):
#   - a generation is judged whole: one file of it written within the age keeps all of it;
#   - a listing that could not be read whole, or an lsof that could not answer, removes nothing;
#   - only names of this crate's own units with a unit hash are ever listed, never another
#     crate's artifact, a lock or a directory of cargo's own;
#   - the moment before removal the process table and lsof are asked again, and each
#     generation's age once more, of that generation alone.
mj_rust_prune_deps_of() {
  mj_pd_d="$1/deps"; mj_pd_age="$2"; mj_pd_stems="$3"
  [ -d "$mj_pd_d" ] || return 0
  mj_pd_say='majordomus: stale crate artifacts'
  mj_pd_find='find'
  [ -x /usr/bin/find ] && mj_pd_find=/usr/bin/find
  set --
  while IFS= read -r mj_pd_s; do
    [ -n "$mj_pd_s" ] || continue
    [ "$#" = 0 ] || set -- "$@" -o
    set -- "$@" -name "$mj_pd_s-*"
  done < "$mj_pd_stems"
  [ "$#" -gt 0 ] || return 0
  mj_pd_tmp="$(mktemp "${TMPDIR:-/tmp}/mj-deps.XXXXXX")" || return 0
  # 1. the listing, old and recent, whole or not at all
  if ! "$mj_pd_find" "$mj_pd_d" -mindepth 1 -maxdepth 1 \( "$@" \) -mmin +"$mj_pd_age" \
        > "$mj_pd_tmp.old" 2> "$mj_pd_tmp.err" \
     || ! "$mj_pd_find" "$mj_pd_d" -mindepth 1 -maxdepth 1 \( "$@" \) ! -mmin +"$mj_pd_age" \
        > "$mj_pd_tmp.new" 2>> "$mj_pd_tmp.err"; then
    if [ ! -s "$mj_pd_tmp.err" ] || grep -v 'No such file or directory' "$mj_pd_tmp.err" | grep -q .; then
      printf '%s: none removed: %s could not be read whole: %s\n' "$mj_pd_say" "$mj_pd_d" \
        "$(head -n 1 "$mj_pd_tmp.err" 2>/dev/null || true)" >&2
      rm -f "$mj_pd_tmp" "$mj_pd_tmp".*; return 0
    fi
  fi
  [ -s "$mj_pd_tmp.old" ] || { rm -f "$mj_pd_tmp" "$mj_pd_tmp".*; return 0; }
  # 2. what is held open, then the plan: `<generation> <path>` for each file of a generation
  #    with no recent and no open file
  if ! mj_rust_open_under "$mj_pd_d" > "$mj_pd_tmp.open"; then
    printf '%s: none removed: what is open in %s could not be read (lsof)\n' "$mj_pd_say" "$mj_pd_d" >&2
    rm -f "$mj_pd_tmp" "$mj_pd_tmp".*; return 0
  fi
  cat "$mj_pd_tmp.new" "$mj_pd_tmp.open" | while IFS= read -r mj_pd_f; do
    mj_rust_generation "$mj_pd_f"
  done | LC_ALL=C sort -u > "$mj_pd_tmp.keep"
  while IFS= read -r mj_pd_f; do
    mj_pd_g="$(mj_rust_generation "$mj_pd_f")"
    [ -n "$mj_pd_g" ] || continue
    grep -qxF -- "$mj_pd_g" "$mj_pd_tmp.keep" && continue
    printf '%s %s\n' "$mj_pd_g" "$mj_pd_f"
  done < "$mj_pd_tmp.old" | LC_ALL=C sort > "$mj_pd_tmp.plan"
  [ -s "$mj_pd_tmp.plan" ] || { rm -f "$mj_pd_tmp" "$mj_pd_tmp".*; return 0; }
  mj_pd_kb="$(cut -d' ' -f2- "$mj_pd_tmp.plan" | tr '\n' '\0' | xargs -0 du -sk 2>/dev/null \
    | awk '{ s += $1 } END { print s + 0 }')"
  mj_pd_gens="$(cut -d' ' -f1 "$mj_pd_tmp.plan" | sort -u | wc -l | tr -d ' ')"
  mj_pd_files="$(wc -l < "$mj_pd_tmp.plan" | tr -d ' ')"
  {
    printf '%s of %s, no file written for over %s min: removing %s generation(s), %s file(s), %s\n' \
      "$mj_pd_say" "$mj_pd_d" "$mj_pd_age" "$mj_pd_gens" "$mj_pd_files" "$(mj_rust_human_kb "$mj_pd_kb")"
    cut -d' ' -f1 "$mj_pd_tmp.plan" | LC_ALL=C sort | uniq -c | head -n 12 \
      | awk '{ printf "  %5d file(s)  %s\n", $1, $2 }'
    [ "$mj_pd_gens" -le 12 ] || printf '  ... and %s more generation(s)\n' "$((mj_pd_gens - 12))"
  } >&2
  # 3. asked again, then removed
  if [ -n "$(mj_rust_profile_users "${mj_pd_d%/deps}" || echo unreadable)" ]; then
    printf '%s: none removed: a process began using %s while the plan was made\n' "$mj_pd_say" "${mj_pd_d%/deps}" >&2
    rm -f "$mj_pd_tmp" "$mj_pd_tmp".*; return 0
  fi
  if ! mj_rust_open_under "$mj_pd_d" > "$mj_pd_tmp.open"; then
    printf '%s: none removed: what is open in %s could not be read again (lsof)\n' "$mj_pd_say" "$mj_pd_d" >&2
    rm -f "$mj_pd_tmp" "$mj_pd_tmp".*; return 0
  fi
  mj_pd_before="$(mj_free_mb "$mj_pd_d" || true)"
  mj_pd_removed=0; mj_pd_changed=0; mj_pd_failed=0; mj_pd_last=''
  while read -r mj_pd_g mj_pd_f; do
    if [ "$mj_pd_g" != "$mj_pd_last" ]; then
      mj_pd_last="$mj_pd_g"; mj_pd_skip=0
      if [ -n "$("$mj_pd_find" "$mj_pd_d" -mindepth 1 -maxdepth 1 \( -name "$mj_pd_g*" -o -name "lib$mj_pd_g*" \) \
              ! -mmin +"$mj_pd_age" 2>/dev/null)" ] \
         || grep -qF -e "/$mj_pd_g" -e "/lib$mj_pd_g" "$mj_pd_tmp.open"; then
        mj_pd_skip=1; mj_pd_changed=$((mj_pd_changed + 1))
      fi
    fi
    [ "$mj_pd_skip" = 0 ] || continue
    if rm -f "$mj_pd_f" 2>/dev/null && [ ! -e "$mj_pd_f" ]; then
      mj_pd_removed=$((mj_pd_removed + 1))
    else
      mj_pd_failed=$((mj_pd_failed + 1))
    fi
  done < "$mj_pd_tmp.plan"
  mj_pd_after="$(mj_free_mb "$mj_pd_d" || true)"
  printf '%s: removed %s file(s); free on its volume %sMB before, %sMB after\n' \
    "$mj_pd_say" "$mj_pd_removed" "${mj_pd_before:-?}" "${mj_pd_after:-?}" >&2
  [ "$mj_pd_changed" = 0 ] || printf '%s: kept %s generation(s) written to or opened since the plan was made\n' \
    "$mj_pd_say" "$mj_pd_changed" >&2
  [ "$mj_pd_failed" = 0 ] || printf '%s: %s file(s) could not be removed\n' "$mj_pd_say" "$mj_pd_failed" >&2
  rm -f "$mj_pd_tmp" "$mj_pd_tmp".*
  return 0
}

# mj_rust_sweep_profile <build-dir> <profile-dir> <mode> <incremental-age> <deps-age> <stems-file>
#
# One profile's sweep, run under cargo's lock on it: nothing is removed while a process names
# the profile in its command line — a test binary, a doctest's rustdoc, a compiler — because
# none of those holds the lock (mj_rust_profile_users); then its idle incremental sessions,
# then, unless <mode> is `incremental`, its stale crate artifacts.
mj_rust_sweep_profile() {
  mj_sp_users="$(mj_rust_profile_users "$2" || echo 'the process table could not be read')"
  if [ -n "$mj_sp_users" ]; then
    printf 'majordomus: %s not swept: in use by %s\n' "$2" \
      "$(printf '%s\n' "$mj_sp_users" | head -n 1 | cut -c1-160)" >&2
    return 0
  fi
  [ "$4" -gt 0 ] && mj_rust_prune_incremental_of "$1" "$2" "$4"
  [ "$3" = incremental ] || [ "$5" -le 0 ] || mj_rust_prune_deps_of "$2" "$5" "$6"
  return 0
}

# mj_rust_prune_build_dir <tree-root> [all|incremental]
#
# What every build of this repository does first: sweep the build directory it is about to
# use, profile by profile (debug, release, each target triple's, and the profiles of a
# coverage run's llvm-cov-target nested inside it), each under cargo's own lock and only while
# no process names it. `incremental` sweeps the idle incremental sessions alone (I2713); `all`
# (the default) the stale crate artifacts as well (I2787).
#
#   - the build directory is cargo's answer or nothing, and unanswered removes nothing;
#   - it must carry cargo's mark, and a profile directory must physically be inside it;
#   - a profile some build holds, or some process names, is skipped and said.
#
# Always returns 0: a sweep that could not run says so, and the build goes on to the floor.
# Once per tree and build directory in a shell, so that a caller that asks the floor and then
# builds through mj_cargo_bounded sweeps once.
mj_rust_prune_build_dir() {
  mj_pb_root="$1"; mj_pb_mode="${2:-all}"
  mj_pb_inc="${MAJORDOMUS_INCREMENTAL_IDLE_MIN:-$MJ_INCREMENTAL_IDLE_MIN_DEFAULT}"
  case "$mj_pb_inc" in ''|*[!0-9]*) mj_pb_inc="$MJ_INCREMENTAL_IDLE_MIN_DEFAULT" ;; esac
  mj_pb_deps="${MAJORDOMUS_DEPS_IDLE_MIN:-$MJ_DEPS_IDLE_MIN_DEFAULT}"
  case "$mj_pb_deps" in ''|*[!0-9]*) mj_pb_deps="$MJ_DEPS_IDLE_MIN_DEFAULT" ;; esac
  [ "$mj_pb_mode" = incremental ] && mj_pb_deps=0
  [ "$mj_pb_inc" -gt 0 ] || [ "$mj_pb_deps" -gt 0 ] || return 0
  mj_pb_key="$mj_pb_root|${CARGO_BUILD_BUILD_DIR:-}|${CARGO_TARGET_DIR:-}"
  [ "${MJ_INCREMENTAL_PRUNED_FOR:-}" = "$mj_pb_key" ] && return 0
  MJ_INCREMENTAL_PRUNED_FOR="$mj_pb_key"

  mj_pb_say='majordomus: the build directory'
  mj_pb_crate="$mj_pb_root/apps/majordomus-cli"
  mj_pb_dir="$(mj_cargo_build_dir "$mj_pb_crate" || true)"
  if [ -z "$mj_pb_dir" ]; then
    printf '%s: none removed: cargo did not say where this build keeps them (cargo metadata failed)\n' \
      "$mj_pb_say" >&2
    return 0
  fi
  [ -d "$mj_pb_dir" ] || return 0   # nothing was ever built there, so there is nothing to reclaim
  if ! mj_rust_is_build_dir "$mj_pb_dir"; then
    printf '%s: none removed: %s carries no mark of cargo'"'"'s, so it is not provably a build directory\n' \
      "$mj_pb_say" "$mj_pb_dir" >&2
    return 0
  fi
  mj_pb_stems="$(mktemp "${TMPDIR:-/tmp}/mj-stems.XXXXXX")" || return 0
  mj_rust_crate_stems "$mj_pb_crate" > "$mj_pb_stems"
  mj_rust_profile_dirs "$mj_pb_dir" > "$mj_pb_stems.profiles"
  while IFS= read -r mj_pb_p; do
    [ -n "$mj_pb_p" ] || continue
    mj_rust_locked "$mj_pb_p" mj_rust_sweep_profile "$mj_pb_dir" "$mj_pb_p" "$mj_pb_mode" \
      "$mj_pb_inc" "$mj_pb_deps" "$mj_pb_stems"
  done < "$mj_pb_stems.profiles"
  rm -f "$mj_pb_stems" "$mj_pb_stems.profiles"
  return 0
}

# ------------------------------------------------------------------------------ private build directories
#
# Where a build goes when the shared one is busy, and how long it stays there.
#
# mj_cargo_bounded waits MAJORDOMUS_BUILD_LOCK_WAIT seconds behind another build's lock on the
# shared build directory and then builds in this tree's own one instead,
# ${CARGO_HOME}/build-private/<cksum of the tree's root> (mj_cargo_private_dir): a cold build
# that compiles every dependency once, a few GiB, and nothing ever removed it. On a machine
# where nine sessions build, most builds wait that long, so every worktree that ever built
# left one behind: on 2026-10-17 they held 178 GiB in 35 directories, 25 of them idle for
# over two hours, the volume reached 3.9 GiB and every session stopped; removing the idle ones
# gave back 150 GiB.
#
#   MAJORDOMUS_PRIVATE_BUILD_IDLE_MIN  minutes nothing in a private build directory may have
#                                      been written before a build removes it whole (default
#                                      below); 0 removes none.
#
# The default is the deps sweep's, for the same reason: removing one too early costs that
# tree one cold build, which is what it would have cost had the shared directory been free.
MJ_PRIVATE_BUILD_IDLE_MIN_DEFAULT=120
# How often one machine looks, at most: the look walks every private directory that is idle,
# all of it, and a build should not pay that every time it starts. A rate limit, never a
# deletion predicate: a stamp that cannot be read means "look now".
MJ_PRIVATE_SWEEP_EVERY_S=600

# mj_rust_private_unheld <build-dir> — is no profile of it held by a build? True (0) when
# every profile's cargo lock could be taken at once; false when one is held, flock(1) is
# missing, or a lock file cannot be opened.
mj_rust_private_unheld() {
  command -v flock >/dev/null 2>&1 || return 1
  for mj_ph_p in $(mj_rust_profile_dirs "$1"); do
    for mj_ph_l in "$mj_ph_p/.cargo-build-lock" "$mj_ph_p/.cargo-lock"; do
      [ -f "$mj_ph_l" ] || continue
      flock -n -x "$mj_ph_l" true 2>/dev/null || return 1
    done
  done
  return 0
}

# mj_rust_reclaim_private
#
# Remove, whole, every private build directory nothing has written to for
# MAJORDOMUS_PRIVATE_BUILD_IDLE_MIN minutes, that no build holds and no process names, and say
# what was reclaimed. A deleting sweep, so it fails closed:
#   - only a real directory directly under build-private carrying cargo's mark, never a
#     symlink, never the one this shell is building in;
#   - one written within the age, held by a build, or named by a running process is kept;
#     a listing or a process table that cannot be read keeps it too;
#   - it is renamed out of cargo's reach before it is removed, and only after the predicates
#     were asked once more: a build that starts afterwards finds no directory and makes a
#     fresh one, never half of an old one.
# Always returns 0.
mj_rust_reclaim_private() {
  mj_rp_age="${MAJORDOMUS_PRIVATE_BUILD_IDLE_MIN:-$MJ_PRIVATE_BUILD_IDLE_MIN_DEFAULT}"
  case "$mj_rp_age" in ''|*[!0-9]*) mj_rp_age="$MJ_PRIVATE_BUILD_IDLE_MIN_DEFAULT" ;; esac
  [ "$mj_rp_age" -gt 0 ] || return 0
  mj_rp_base="${CARGO_HOME:-$HOME/.cargo}/build-private"
  [ -d "$mj_rp_base" ] || return 0
  mj_rp_stamp="${XDG_CACHE_HOME:-$HOME/.cache}/majordomus/private-sweep.stamp"
  mj_rp_last="$(cat "$mj_rp_stamp" 2>/dev/null || true)"
  case "$mj_rp_last" in ''|*[!0-9]*) mj_rp_last=0 ;; esac
  mj_rp_now="$(date +%s)"
  [ $((mj_rp_now - mj_rp_last)) -ge "$MJ_PRIVATE_SWEEP_EVERY_S" ] || return 0
  mkdir -p "$(dirname "$mj_rp_stamp")" 2>/dev/null && printf '%s\n' "$mj_rp_now" > "$mj_rp_stamp" 2>/dev/null || true
  mj_rp_find='find'
  [ -x /usr/bin/find ] && mj_rp_find=/usr/bin/find
  mj_rp_say='majordomus: idle private build directories'
  mj_rp_before="$(mj_free_mb "$mj_rp_base" || true)"
  mj_rp_removed=''; mj_rp_kept=0
  mj_rp_err="$(mktemp "${TMPDIR:-/tmp}/mj-private.XXXXXX")" || return 0
  for mj_rp_d in "$mj_rp_base"/*; do
    [ -d "$mj_rp_d" ] && [ ! -L "$mj_rp_d" ] || continue
    [ "$mj_rp_d" != "${CARGO_BUILD_BUILD_DIR:-}" ] || { mj_rp_kept=$((mj_rp_kept + 1)); continue; }
    mj_rust_is_build_dir "$mj_rp_d" || { mj_rp_kept=$((mj_rp_kept + 1)); continue; }
    mj_rp_ok=1
    for _ in 1 2; do
      # a listing with a hole is no proof of idleness: an error keeps the directory
      mj_rp_recent="$("$mj_rp_find" "$mj_rp_d" -mmin -"$mj_rp_age" -print 2>"$mj_rp_err" | head -n 1)"
      [ ! -s "$mj_rp_err" ] || mj_rp_recent=unreadable
      [ -z "$mj_rp_recent" ] || { mj_rp_ok=0; break; }
      [ -z "$(mj_rust_profile_users "$mj_rp_d" || echo unreadable)" ] || { mj_rp_ok=0; break; }
      mj_rust_private_unheld "$mj_rp_d" || { mj_rp_ok=0; break; }
    done
    [ "$mj_rp_ok" = 1 ] || { mj_rp_kept=$((mj_rp_kept + 1)); continue; }
    mj_rp_gone="$mj_rp_base/.reclaiming-${mj_rp_d##*/}-$$"
    if mv "$mj_rp_d" "$mj_rp_gone" 2>/dev/null; then
      rm -rf "$mj_rp_gone" 2>/dev/null || true   # renamed out of cargo's reach a line above
      mj_rp_removed="$mj_rp_removed ${mj_rp_d##*/}"
    else
      mj_rp_kept=$((mj_rp_kept + 1))
    fi
  done
  rm -f "$mj_rp_err"
  if [ -n "$mj_rp_removed" ]; then
    printf '%s under %s, nothing written for over %s min, held and named by nothing: removed%s; kept %s; free %sMB before, %sMB after\n' \
      "$mj_rp_say" "$mj_rp_base" "$mj_rp_age" "$mj_rp_removed" "$mj_rp_kept" \
      "${mj_rp_before:-?}" "$(mj_free_mb "$mj_rp_base" || echo '?')" >&2
  fi
  return 0
}

# mj_rust_prune_incremental <tree-root> — the idle incremental sessions alone (I2713).
mj_rust_prune_incremental() {
  mj_rust_prune_build_dir "$1" incremental
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
  done | LC_ALL=C sort -u | while IFS= read -r mj_lb_d; do
    printf '%s\t%s\n' "$(mj_rust_kb "$mj_lb_d" || printf '%s' -1)" "$mj_lb_d"
  done | LC_ALL=C sort -rn | head -n 5 | while IFS="$(printf '\t')" read -r mj_lb_kb mj_lb_d; do
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

# How long a build behind the lock keeps waiting when the disk is too low to go private, in
# seconds, before it refuses (I3839).
MJ_BUILD_LOCK_WAIT_LOW_DISK_DEFAULT=1800

# mj_cargo_private_dir <tree-root>
#
# The build directory that is this tree's alone, under cargo's home like the shared one, so
# that it is one path whatever the tree's depth. Named by the tree's root, which is what
# makes two worktrees two directories.
mj_cargo_private_dir() {
  printf '%s/build-private/%s\n' "${CARGO_HOME:-$HOME/.cargo}" \
    "$(printf '%s' "$1" | cksum | awk '{ print $1 }')"
}

# mj_build_cache_start <tree-root>
#
# Give this build the mesh build cache, once, when an sccache server is about to start
# (project.every-build-path-goes-through-the-cache, ADR 0157, I3953).
#
# sccache fixes its levels when its server starts and fails a compile when an S3 level is
# down at that moment, so the store is asked about exactly then and never per compilation:
#   - a server already listening on its port is left alone (a connect, no probe, no process);
#   - otherwise `majordomus mesh build-cache env` of this tree's executable derives the
#     configuration (store healthy, credential file private, sccache with the chain) and the
#     server is started with it, in a subshell so the variables reach the server and nothing
#     else; only the credential file's path is ever in the environment, never a key;
#   - then, and only then, cargo is pointed at sccache (RUSTC_WRAPPER) unless the caller
#     already named a wrapper, including an empty one.
# The store down, no sccache, no executable (a cold node), no nc to see the port, or
# MAJORDOMUS_BUILD_CACHE=off: the build is exactly what it was, and one line says why. This
# never fails a build; it returns 0 always, and once per process.
mj_build_cache_start() {
  [ -z "${MJ_BUILD_CACHE_DONE:-}" ] || return 0
  MJ_BUILD_CACHE_DONE=1
  [ "${MAJORDOMUS_BUILD_CACHE:-}" = off ] && return 0
  command -v sccache >/dev/null 2>&1 || return 0
  [ -z "${RUSTC_WRAPPER+x}" ] && [ -z "${RUSTC_WORKSPACE_WRAPPER+x}" ] || return 0
  mj_bc_bin="$(mj_rust_bin "$1" 2>/dev/null || true)"
  if [ ! -x "$mj_bc_bin" ]; then
    printf 'majordomus: build cache: local-only: no majordomus executable yet (a cold node builds as before)\n' >&2
    return 0
  fi
  command -v nc >/dev/null 2>&1 || { printf 'majordomus: build cache: local-only: no nc to see whether an sccache server runs\n' >&2; return 0; }
  mj_bc_port="${SCCACHE_SERVER_PORT:-4226}"
  if nc -z -w 1 127.0.0.1 "$mj_bc_port" >/dev/null 2>&1; then
    # a running server has its levels: ask nothing, probe nothing
    RUSTC_WRAPPER=sccache; export RUSTC_WRAPPER
    return 0
  fi
  mj_bc_env="$("$mj_bc_bin" mesh build-cache env --sccache sccache 2>/dev/null || true)"
  case "$mj_bc_env" in
    *"export "*)
      if ( eval "$mj_bc_env" && sccache --start-server ) >/dev/null 2>&1; then
        RUSTC_WRAPPER=sccache; export RUSTC_WRAPPER
      else
        printf 'majordomus: build cache: local-only: sccache did not start with the store\n' >&2
      fi ;;
    *)
      printf 'majordomus: build cache: %s\n' \
        "$(printf '%s\n' "$mj_bc_env" | sed -n 's/^# build cache: //p' | head -n 1 | grep . || echo 'local-only: majordomus could not derive a configuration')" >&2 ;;
  esac
  return 0
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
#   - but not below the high-water mark (MAJORDOMUS_PRIVATE_BUILD_ABOVE_MB, by default the
#     reclaim mark MJ_RECLAIM_BELOW_MB_DEFAULT): a cold private build compiles every
#     dependency once (5.3 GB, measured 2026-10-21 while 32-35 GiB were free), and with
#     several builds waiting every one of them goes private at once, which multiplies the
#     disk use exactly when the disk is lowest (I3839). There it waits on the shared lock
#     again, up to MAJORDOMUS_BUILD_LOCK_WAIT_LOW_DISK seconds in all, and then refuses with
#     exit 75, naming the free space, the mark and the other cargo processes;
#   - a build that is already in a directory of its own (CARGO_BUILD_BUILD_DIR set by the
#     caller) and is still blocked at the bound has no second place to go: it stops with
#     exit 75 and says what holds it, rather than wait for ever.
#
# Exit status: cargo's own; 12 when the disk floor refuses the private build; 75 when the
# bound was reached with nowhere left to build, or below the high-water mark.
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
  mj_rust_prune_build_dir "$mj_cb_root"
  mj_build_cache_start "$mj_cb_root"
  mj_cb_log="$(mktemp "${TMPDIR:-/tmp}/mj-cargo.XXXXXX")" || return 12
  mj_cb_private=0
  [ -n "${CARGO_BUILD_BUILD_DIR:-}" ] && mj_cb_private=1
  mj_cb_mark="${MAJORDOMUS_PRIVATE_BUILD_ABOVE_MB:-$MJ_RECLAIM_BELOW_MB_DEFAULT}"
  case "$mj_cb_mark" in ''|*[!0-9]*) mj_cb_mark="$MJ_RECLAIM_BELOW_MB_DEFAULT" ;; esac
  mj_cb_low_wait="${MAJORDOMUS_BUILD_LOCK_WAIT_LOW_DISK:-$MJ_BUILD_LOCK_WAIT_LOW_DISK_DEFAULT}"
  case "$mj_cb_low_wait" in ''|*[!0-9]*) mj_cb_low_wait="$MJ_BUILD_LOCK_WAIT_LOW_DISK_DEFAULT" ;; esac
  mj_cb_low_waited=0
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
    # below the high-water mark a private build would deepen the shortage it meets: wait on
    # the shared lock again, bounded, and refuse rather than go private (I3839)
    if [ "$mj_cb_mark" -gt 0 ]; then
      mj_cb_target="$(mj_cargo_target_dir "$mj_cb_root/apps/majordomus-cli")"
      mj_cb_free="$(mj_free_mb "$mj_cb_target" || true)"
      if [ -n "$mj_cb_free" ] && [ "$mj_cb_free" -lt "$mj_cb_mark" ]; then
        mj_cb_low_waited=$((mj_cb_low_waited + mj_cb_blocked))
        if [ "$mj_cb_low_waited" -lt "$mj_cb_low_wait" ]; then
          printf 'majordomus: %sMB free is under %sMB, where a cold private build would deepen the shortage; waiting on the shared lock again (%ss of %ss)\n' \
            "$mj_cb_free" "$mj_cb_mark" "$mj_cb_low_waited" "$mj_cb_low_wait" >&2
          continue
        fi
        printf 'majordomus: refusing a private build: %sMB free is under %sMB, and the shared build directory stayed locked for %ss; other cargo processes: %s. Wait for them, free space, or set MAJORDOMUS_PRIVATE_BUILD_ABOVE_MB=0 to build privately anyway\n' \
          "$mj_cb_free" "$mj_cb_mark" "$mj_cb_low_waited" "${mj_cb_others:-none found}" >&2
        rm -f "$mj_cb_log"
        return 75
      fi
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

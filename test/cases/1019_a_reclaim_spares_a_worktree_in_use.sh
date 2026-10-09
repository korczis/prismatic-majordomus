# majordomus-covers: none
# A reclaim spares a worktree a derive is building, whether or not a process stands in it.
#
# On 2026-10-09 d1's derive of #831 lost its whole apps/majordomus-cli/target between stages B
# and C. The machine lock's owner line named that worktree; the reaper read only `ps` command
# lines and working directories, and a derive between two stages, or cargo given
# --manifest-path from elsewhere, shows in neither. The lock is now the third reading of who is
# working (I2109):
#
#   - the owner line lib/machine_lock.sh writes, "<who>:<branch> <pid>", names the worktree
#     that has <branch> checked out; the line and the lock directory are taken from that file
#     itself, so a change of its format or its place breaks this case;
#   - an owner line written by hand names it by `worktree=<path>`, or by its branch as a bare
#     word or `branch=<b>`;
#   - a held lock whose owner line names no worktree of this repository, or is empty, spares
#     every worktree: unknown is not idle;
#   - a live process standing in a worktree still spares it, and each keep says which reading
#     decided it;
#   - with the lock gone, the same worktrees are reclaimed, so the lock was the only reason.
#
# Hermetic: a fixture repository and its own worktrees under $T. The lock lives where the
# reaper looks for it by default — beside the fixture's primary checkout — so nothing here
# reads or touches the machine's own ~/dev/.derive.lock.
. "$ROOT/test/lib.sh"

REAPER="$ROOT/scripts/reap-orphans"
expect_file "$REAPER"
command -v lsof >/dev/null 2>&1 || skip "no lsof: the reaper refuses without it"
unset MAJORDOMUS_DERIVE_LOCK

FIX="$T/fix"
WTS="$T/fix-wt"
LOCK="$T/.derive.lock"   # beside $FIX, the primary checkout
mkdir -p "$FIX" "$WTS"
(
  cd "$FIX"
  git init -q .
  git config user.email t@example.com
  git config user.name Test
  mkdir -p apps/majordomus-cli/src
  echo 'fn main(){}' > apps/majordomus-cli/src/main.rs
  echo 'target' > .gitignore
  git add -A && git commit -qm base
  git update-ref refs/remotes/origin/master HEAD
) || { echo "    the fixture repository could not be built"; exit 2; }

mkbuild() {
  mkdir -p "$1/apps/majordomus-cli/target/debug"
  printf 'Signature: 8a477f597d28d172789f06886806bc55\n# This file is a cache directory tag created by cargo.\n' \
    > "$1/apps/majordomus-cli/target/CACHEDIR.TAG"
  echo build-output > "$1/apps/majordomus-cli/target/MARKER"
}
addwt() { git -C "$FIX" worktree add -q -b "$1" "$WTS/$1" >/dev/null 2>&1 || return 1; mkbuild "$WTS/$1"; }
for w in idle by-lib by-path by-word by-branch-key live; do
  addwt "$w" || { echo "    could not add the $w worktree"; exit 2; }
done
built() { [ -f "$WTS/$1/apps/majordomus-cli/target/MARKER" ]; }
# every sweep starts from a full set: what a run reclaims is put back before the next one
rebuild() { for w in idle by-lib by-path by-word by-branch-key live; do built "$w" || mkbuild "$WTS/$w"; done; }

# a process that stands in `live` and names nothing
( cd "$WTS/live" && exec sleep 300 ) >/dev/null 2>&1 &
LIVE=$!
trap 'kill "$LIVE" 2>/dev/null || true' EXIT
sleep 1

reclaim() {  # reclaim <report file>
  rc=0
  MJ_ROOT="$FIX" "$REAPER" --targets --reclaim > "$1" 2>&1 || rc=$?
  LAST_OUT="$(cat "$1")"
  [ "$rc" = 0 ] || { echo "    the sweep exited $rc, not 0:"; sed 's/^/      /' "$1"; exit 1; }
}
own() { mkdir -p "$LOCK" && printf '%s\n' "$1" > "$LOCK/owner"; }

# ---------------------------------------------------------------- as the derive writes it
# The owner line and the lock's place come from lib/machine_lock.sh (case 1018), never from a
# copy of its format here. Until that file is on this branch's base a hand-written line in
# its format stands in, and the case says so.
LIB="$ROOT/lib/machine_lock.sh"
if [ -f "$LIB" ]; then
  # shellcheck source=../../lib/machine_lock.sh
  LIB_OUT="$( . "$LIB" && mj_lock_init "$WTS/by-lib" derive \
    && printf '%s\n%s\n' "$MJ_LOCK_DIR" "$MJ_LOCK_LINE" )" \
    || { echo "    lib/machine_lock.sh could not be sourced or mj_lock_init failed"; exit 1; }
  LIB_DIR="$(printf '%s\n' "$LIB_OUT" | sed -n 1p)"
  LIB_LINE="$(printf '%s\n' "$LIB_OUT" | sed -n 2p)"
else
  echo "    (lib/machine_lock.sh is not on this base yet: a hand-written line in its format stands in)"
  LIB_DIR="$LOCK"; LIB_LINE="derive:by-lib 999999"
fi
mkdir -p "$LIB_DIR" && printf '%s\n' "$LIB_LINE" > "$LIB_DIR/owner"
reclaim "$T/by-lib.txt"
expect_grep 'by-lib.*keep — merged, but somebody is working in it \(the derive lock names it\)'
built by-lib || { echo "    the worktree the derive's own owner line names lost its build output ($LIB_LINE)"; exit 1; }
built idle && { echo "    the idle worktree was kept although the lock names another"; exit 1; }
rm -rf "$LIB_DIR"
echo "    the owner line the derive writes, <who>:<branch> <pid>, spares that branch's worktree"

# ---------------------------------------------------------------- the lock names a path
rebuild
own "derive pid=999999 branch=by-path worktree=$WTS/by-path at=2026-10-09T09:03:00Z"
reclaim "$T/by-path.txt"
expect_grep 'by-path.*keep — merged, but somebody is working in it \(the derive lock names it\)'
built by-path || { echo "    the worktree the lock names lost its build output"; exit 1; }
expect_grep 'live.*keep — merged, but somebody is working in it \(a live process\)'
built live || { echo "    the worktree a process stands in lost its build output"; exit 1; }
expect_grep 'idle.*reclaimed'
built idle && { echo "    the idle worktree was kept although nothing names it"; exit 1; }
echo "    worktree=<path> spares that worktree, a live process spares its own, and the rest is reclaimed"

# ---------------------------------------------------------------- by branch, written by hand
rebuild
own "d1 by-word repair 10:50"
reclaim "$T/by-word.txt"
expect_grep 'by-word.*keep — .*\(the derive lock names it\)'
built by-word || { echo "    the worktree whose branch the owner line names lost its build output"; exit 1; }
built idle && { echo "    a lock naming one branch spared another"; exit 1; }

rebuild
own "figure branch=by-branch-key 56337"
reclaim "$T/by-branch-key.txt"
expect_grep 'by-branch-key.*keep — .*\(the derive lock names it\)'
built by-branch-key || { echo "    branch=<b> did not name its worktree"; exit 1; }
echo "    an owner line written by hand names its worktree by branch, bare or as branch=<b>"

# ---------------------------------------------------------------- unknown is not idle
rebuild
own "p1-handover-mod 99469"
reclaim "$T/unresolved.txt"
expect_grep 'names no worktree of this repository: p1-handover-mod 99469'
expect_grep 'nothing is reclaimed'
expect_grep 'idle.*keep — merged, but a derive holds the lock and names no worktree'
expect_grep 'reclaimed 0'
built idle || { echo "    a lock naming no worktree let the reaper reclaim one"; exit 1; }

: > "$LOCK/owner"
reclaim "$T/empty.txt"
expect_grep 'owner line is empty or unreadable'
built idle || { echo "    a lock with an empty owner line let the reaper reclaim"; exit 1; }
echo "    a held lock that names no worktree, or nothing at all, spares every worktree"

# ---------------------------------------------------------------- and without the lock
rm -rf "$LOCK"
rebuild
reclaim "$T/free.txt"
expect_no_grep 'derive lock'
for w in idle by-lib by-path by-word by-branch-key; do
  built "$w" && { echo "    $w was kept with no lock and nobody in it"; sed 's/^/      /' "$T/free.txt"; exit 1; }
done
built live || { echo "    the worktree a process stands in lost its build output"; exit 1; }
expect_file "$WTS/by-path/apps/majordomus-cli/src/main.rs"
echo "    with the lock released the same worktrees are reclaimed: the lock was the only reason"

echo "    ok: a reclaim spares a worktree in use, by the derive lock or by a live process"

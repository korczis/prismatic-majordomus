# claims: none
# The executable resolver borrows an executable another worktree of this checkout already
# built, when it is of this tree's generation, before it refuses (I4618). On 2026-11-15 a
# change to one shell file (lib/check.sh, I4598) cost an 18-minute cold build and 3.4 GiB
# because the commit hook found no executable in its own worktree, while other worktrees held
# a current one of the same crate; a peer had borrowed one by hand the day before.
#
# A copy of lib/rust_bin.sh runs in a small repository whose crate is one file, with one other
# worktree whose stand-in executable is judged:
#
#   1  current, its crate clean and equal to ours: borrowed, and the worktree named on stderr
#   2  its crate edited and not committed: not borrowed, named in the refusal with the reason
#   3  older than its own sources: not borrowed, with the reason
#   4  its branch carries another crate: not borrowed, with the reason
#   5  no other worktree holds an executable: the refusal says so
#   6  a shared target directory: no worktree is asked, and the refusal says why
#
# and in none of them is anything built.
. "$ROOT/test/lib.sh"
unset MAJORDOMUS_BIN MAJORDOMUS_BIN_COMMIT CARGO_TARGET_DIR CARGO_BUILD_TARGET_DIR
export MAJORDOMUS_INSTALLED_BIN="$T/no-installed-release"

fail() { printf '    %s\n' "$*"; exit 1; }
mkdir -p repo/lib repo/apps/majordomus-cli/src
cp "$ROOT/lib/rust_bin.sh" repo/lib/rust_bin.sh
printf '[package]\nname = "fixture"\n' > repo/apps/majordomus-cli/Cargo.toml
printf 'fn main() {}\n' > repo/apps/majordomus-cli/src/main.rs
printf 'target/\n' > repo/.gitignore
gitc() { git -C "$1" -c user.email=case@example.invalid -c user.name=case "${@:2}"; }
git -C repo init -q && git -C repo add -A && gitc repo commit -qm crate || fail "could not build the fixture repository"
git -C repo worktree add -q -b sibling "$T/sib" >/dev/null 2>&1 || fail "could not add the other worktree"
# git names a worktree by its resolved path (/var is /private/var on macOS); so does this case
SIB="$(cd "$T/sib" && pwd -P)"
SIB_EXE="$SIB/apps/majordomus-cli/target/debug/majordomus"

stand_in() {
  mkdir -p "$(dirname "$SIB_EXE")"
  printf '#!/usr/bin/env bash\necho sibling\n' > "$SIB_EXE"; chmod +x "$SIB_EXE"
  touch -t 203001010000 "$SIB_EXE"   # newer than every source of its tree
}
# resolve: the resolver of the copy, run from the fixture's own worktree, which has no executable
resolve() {
  ( cd "$T/repo" && . lib/rust_bin.sh && mj_rust_resolve "$T/repo" 'x: ' ) > "$T/out" 2> "$T/err"
}

# ---------------------------------------------------------------- 1. borrowed
stand_in
resolve || fail "a current executable of the same crate in the other worktree was not borrowed: $(cat "$T/err")"
[ "$(cat "$T/out")" = "$SIB_EXE" ] || fail "the resolver printed $(cat "$T/out"), not $SIB_EXE"
grep -q "answering through $SIB_EXE, built in the worktree $SIB" "$T/err" \
  || fail "the borrowed worktree is not named: $(cat "$T/err")"

# ---------------------------------------------------------------- 2. its crate edited, uncommitted
printf 'fn main() { println!("x"); }\n' > "$T/sib/apps/majordomus-cli/src/main.rs"
touch -t 203001010000 "$SIB_EXE"
resolve && fail "an executable whose worktree is editing its crate was borrowed"
grep -q "not borrowed from $SIB: its crate has changes not committed" "$T/err" \
  || fail "the refusal does not name the dirty crate: $(cat "$T/err")"
grep -q 'run: just build' "$T/err" || fail "the refusal does not name the explicit build: $(cat "$T/err")"
git -C "$T/sib" checkout -q -- apps/majordomus-cli/src/main.rs

# ---------------------------------------------------------------- 3. older than its own sources
touch -t 202001010000 "$SIB_EXE"
resolve && fail "an executable older than its own sources was borrowed"
grep -q "not borrowed from $SIB: older than its own sources" "$T/err" \
  || fail "the refusal does not name the stale executable: $(cat "$T/err")"

# ---------------------------------------------------------------- 4. another crate on its branch
printf 'fn main() { let _ = 1; }\n' > "$T/sib/apps/majordomus-cli/src/main.rs"
gitc "$T/sib" commit -qam "another crate" || fail "could not commit on the other worktree"
touch -t 203001010000 "$SIB_EXE"
resolve && fail "an executable of another crate was borrowed"
grep -q "not borrowed from $SIB: the crate differs from" "$T/err" \
  || fail "the refusal does not name the other crate: $(cat "$T/err")"

# ---------------------------------------------------------------- 5. no executable anywhere else
rm -f "$SIB_EXE"
resolve && fail "something was borrowed with no executable in any other worktree"
grep -q 'no other worktree of this checkout holds an executable to borrow' "$T/err" \
  || fail "the refusal does not say no worktree holds one: $(cat "$T/err")"

# ---------------------------------------------------------------- 6. a shared target directory
stand_in
gitc "$T/sib" reset -q --hard HEAD~1   # the same crate again
touch -t 203001010000 "$SIB_EXE"
( export CARGO_TARGET_DIR="$T/shared-target"; resolve ) && fail "a worktree was borrowed from under a shared target directory"
grep -q 'no other worktree was asked: a shared target directory names one executable for all of them' "$T/err" \
  || fail "the refusal does not say why no worktree was asked: $(cat "$T/err")"

# and nothing in any step built
[ ! -d "$T/repo/apps/majordomus-cli/target" ] || fail "a target directory appeared in the fixture's own worktree"
exit 0

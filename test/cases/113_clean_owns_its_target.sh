# Cleaning removes this checkout's build directory and no other one's, and a missing
# executable says that it is a missing executable.
#
# Both halves are the same incident. `cargo clean --manifest-path X` does not clean X's
# target directory: with CARGO_TARGET_DIR set it cleans that, and the manifest decides
# nothing at all — which is exactly how several worktrees of this repository share one build
# directory. So the one recipe a worker runs to reclaim their own disk took everybody else's
# executable with it, twice on 2026-09-10, while their own target/ survived untouched. What
# the borrowers saw was `MAJORDOMUS_BIN is not an executable` in the middle of a suite, and
# what they concluded was that the branch they were measuring was broken.
#
# The rule is `project.reclaim-only-what-you-own`, and it is advisory because no gate can
# tell a careless deletion from a deliberate one. This case holds the part that is not a
# judgement call: a recipe may not remove a directory outside the checkout it was invoked
# from *by accident*, and a caller pointed at an executable that is gone must say whose
# fault that is not.
. "$ROOT/test/lib.sh"

RB="$ROOT/lib/rust_bin.sh"
expect_file "$RB"
expect_file "$ROOT/.just/build.just"

# --- the recipe does not hand cargo a manifest and hope ------------------------------------
# A bare `cargo clean --manifest-path` in the declaration is the defect itself. The recipe
# must go through the resolution that knows where cargo would really clean.
JF="$ROOT/.just/build.just"
if grep -qE '^ +cargo clean --manifest-path' "$JF"; then
  echo "    .just/build.just cleans by manifest path alone, which cleans CARGO_TARGET_DIR"
  echo "    — another checkout's build directory — whenever that variable is set"
  exit 1
fi
expect_grep 'mj_rust_clean' "$JF"

# --- a fixture of two checkouts ------------------------------------------------------------
# Two trees, each with a build directory holding a marker. Nothing here is a real crate build:
# what is under test is which directory the deletion lands in.
mkchk() {
  mkdir -p "$1/apps/majordomus-cli/src" "$1/apps/majordomus-cli/target/debug"
  printf '[package]\nname = "c"\nversion = "0.0.0"\nedition = "2021"\n\n[[bin]]\nname = "majordomus"\npath = "src/main.rs"\n' \
    > "$1/apps/majordomus-cli/Cargo.toml"
  echo 'fn main(){}' > "$1/apps/majordomus-cli/src/main.rs"
  echo marker > "$1/apps/majordomus-cli/target/MARKER"
}
MINE="$T/mine"; THEIRS="$T/theirs"
mkchk "$MINE"; mkchk "$THEIRS"
. "$RB"

# --- it refuses when the build directory is somebody else's --------------------------------
# The incident, exactly: standing in my own checkout, with the variable that borrows another
# one's build directory still exported.
rc=0
out="$(CARGO_TARGET_DIR="$THEIRS/apps/majordomus-cli/target" mj_rust_clean "$MINE" 2>&1)" || rc=$?
[ "$rc" = 12 ] || { echo "    cleaning with a foreign CARGO_TARGET_DIR exited $rc, not 12 (refused)"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
[ -f "$THEIRS/apps/majordomus-cli/target/MARKER" ] || { echo "    it removed the other checkout's build directory"; exit 1; }
[ -f "$MINE/apps/majordomus-cli/target/MARKER" ] || { echo "    it removed this checkout's build directory while refusing"; exit 1; }
# and it says which directory, and what named it, because neither is in the command typed
LAST_OUT="$out"
expect_grep 'refusing to clean'
expect_grep 'CARGO_TARGET_DIR'
# the tail only: the diagnostic prints physical paths, and $TMPDIR here is a symlink
expect_grep 'theirs/apps/majordomus-cli/target'

# --- a symlink out of the checkout is not ownership ----------------------------------------
# The other way worktrees have shared one build directory here. The bytes named are somebody
# else's whichever side of the checkout boundary the name sits on.
if command -v cargo >/dev/null 2>&1; then
  rm -rf "$MINE/apps/majordomus-cli/target"
  mkdir -p "$T/shared"; echo marker > "$T/shared/MARKER"
  ln -s "$T/shared" "$MINE/apps/majordomus-cli/target"
  rc=0
  out="$( unset CARGO_TARGET_DIR; mj_rust_clean "$MINE" 2>&1 )" || rc=$?
  [ "$rc" = 12 ] || { echo "    a target/ symlinked out of the checkout exited $rc, not 12 (refused)"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
  [ -f "$T/shared/MARKER" ] || { echo "    it followed the symlink and removed the shared directory"; exit 1; }
  rm -f "$MINE/apps/majordomus-cli/target"
  mkdir -p "$MINE/apps/majordomus-cli/target"; echo marker > "$MINE/apps/majordomus-cli/target/MARKER"

  # --- and it still cleans its own -------------------------------------------------------
  # A guard that refuses everything is not a guard; the recipe has a job.
  rc=0
  out="$( unset CARGO_TARGET_DIR; cd "$MINE" && mj_rust_clean "$MINE" 2>&1 )" || rc=$?
  [ "$rc" = 0 ] || { echo "    cleaning its own build directory exited $rc"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
  if [ -e "$MINE/apps/majordomus-cli/target/MARKER" ]; then echo "    it did not clean its own build directory"; exit 1; fi
  [ -f "$THEIRS/apps/majordomus-cli/target/MARKER" ] || { echo "    cleaning its own took the other checkout's too"; exit 1; }
else
  echo "    (no cargo: the symlink and the clean-its-own halves are skipped)"
fi

# --- a missing executable says it is a missing executable ----------------------------------
# The sentence is the whole point. A reader who takes this for a failure of the code under
# test loses an afternoon on a branch that was never broken — which is what happened.
LAST_OUT="$(MAJORDOMUS_BIN="$THEIRS/apps/majordomus-cli/target/debug/majordomus" \
  mj_rust_bin_missing "$MINE" "$THEIRS/apps/majordomus-cli/target/debug/majordomus" '    ' 2>&1)"
expect_grep 'is not an executable'
expect_grep 'The tree is not broken'
expect_grep 'nothing was measured and nothing was judged'
expect_grep 'outside this checkout'
expect_grep 'cargo build --manifest-path'

# --- and every caller that resolves one says it that way -----------------------------------
# Two callers resolve the executable and both used to write their own sentence; a sentence
# written twice is a sentence that drifts, and the launcher's wording is matched by case 90.
expect_grep 'mj_rust_bin_missing' "$ROOT/bin/majordomus-cli"
expect_grep 'mj_rust_bin_missing' "$ROOT/test/lib.sh"
expect_no_grep 'MAJORDOMUS_BIN is not an executable' "$ROOT/test/lib.sh"

echo "    ok: cleaning stops at the checkout boundary, and a missing executable says so"

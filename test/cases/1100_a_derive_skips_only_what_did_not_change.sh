# majordomus-covers: none
# claims: derivation-one-graph
# majordomus-timeout: 300
# An incremental derive leaves stage B alone only when a full derive would change nothing
# (ADR 0143, project.derived-files-regenerated): the differential oracle in its smallest form.
#
# Two clones of one fixture repository, each with the repository's own scripts/derive and its
# libraries, unmodified. Stage A and C are a stand-in `majordomus generate`; stage B is a
# stand-in site generator that declares its outputs through `--outputs`, as the real one does,
# and reads one input any input list would name and one none would. The decision is the real
# executable's: `derive begin` and `derive record` are handed to it. One clone derives with
# --incremental, the other with --full, after the same edit; after each, the two trees must be
# byte for byte the same (every file but .git), and the incremental one must have run stage B
# exactly when something it reads moved:
#
#   no-op ................................ skipped, equal
#   a tracked input any list would name .. ran, equal
#   a tracked input no list names ........ ran, equal
#   a deleted output ..................... ran, equal (restored)
#   a tampered output .................... ran, equal (restored)
#   a staged new file .................... ran, equal (the link list moved)
#   an unstaged tracked change ........... ran, equal
#   a test of the crate edited ........... skipped, equal (it reaches only `cargo test`)
#   a failed stage B ..................... nothing recorded; the next derive runs it
#   an input outside every key ........... --shadow keeps the full bytes and exits 13
#
# The stand-in executable forwards `derive` to the real one with MAJORDOMUS_BIN pointing at
# it, which is what scripts/derive exports for the scenarios; the stand-in stage B runs none.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
fail() { printf '    %s\n' "$*"; exit 1; }

# ------------------------------------------------------------------ the real generator declares its outputs
# asked of a copy alone in its own directory, so that a derive running in this checkout cannot
# be mistaken for what the question wrote: the answer needs nothing beside the script
mkdir -p "$T/gen/scripts"; cp "$ROOT/scripts/generate-site-data" "$T/gen/scripts/"
OUTS="$("$T/gen/scripts/generate-site-data" --outputs)" || fail "generate-site-data --outputs exited non-zero"
for want in 'site/data/generated/' 'site/content/'; do
  printf '%s\n' "$OUTS" | grep -qxF "$want" || fail "generate-site-data --outputs does not name $want"
done
# every document its publish block copies into docs/, read out of the block itself
for doc in $(grep -oE 'cp "\$STAGE/[A-Z_]+\.md" "\$ROOT/docs/[A-Z_]+\.md' "$ROOT/scripts/generate-site-data" | sed 's#.*\$ROOT/##'); do
  printf '%s\n' "$OUTS" | grep -qxF "$doc" || fail "generate-site-data publishes $doc and --outputs does not name it"
done
[ "$(ls -A "$T/gen")" = scripts ] || fail "asking generate-site-data for its outputs wrote beside it: $(ls -A "$T/gen" | tr '\n' ' ')"

# ------------------------------------------------------------------ the fixture
BASE="$T/base"; TOOL="$T/tool"; LOG="$T/calls.log"
mkdir -p "$BASE/scripts" "$BASE/lib" "$BASE/src" "$BASE/out/site" "$BASE/docs" "$BASE/apps/majordomus-cli/tests" "$TOOL"
cp "$ROOT/scripts/derive" "$BASE/scripts/"
cp "$ROOT/lib/rust_bin.sh" "$ROOT/lib/machine_lock.sh" "$BASE/lib/"
printf 'v1\n' > "$BASE/src/model.txt"
printf 'helper one\n' > "$BASE/lib/helper.sh"
printf '#[test] fn t() {}\n' > "$BASE/apps/majordomus-cli/tests/t.rs"
: > "$BASE/out/a.txt"; : > "$BASE/out/c.txt"; : > "$BASE/out/site/page.txt"; : > "$BASE/out/site/links.txt"
: > "$BASE/docs/STATUS.md"; : > "$BASE/.gitattributes"
printf '.ai/local/\n' > "$BASE/.gitignore"

cat > "$TOOL/majordomus" <<'SH'
#!/usr/bin/env bash
set -eu
if [ "${1:-}" = derive ]; then
  printf 'derive %s\n' "$2" >> "$STUB_LOG"
  MAJORDOMUS_BIN="$REAL_BIN" exec "$REAL_BIN" "$@"
fi
repo=""; while [ $# -gt 0 ]; do case "$1" in --repo) repo="$2"; shift 2 ;; *) shift ;; esac; done
printf 'generate\n' >> "$STUB_LOG"
cd "$repo"
printf 'a(%s)\n' "$(cat src/model.txt)" > out/a.txt
printf 'c(%s)\n' "$(cat src/model.txt)" > out/c.txt
SH
cat > "$BASE/scripts/generate-site-data" <<'SH'
#!/usr/bin/env bash
set -eu
cd "$(dirname "$0")/.."
if [ "${1:-}" = --outputs ]; then printf 'out/site/\ndocs/STATUS.md\n'; exit 0; fi
printf 'site-data\n' >> "$STUB_LOG"
[ -z "${STUB_FAIL:-}" ] || { echo "generate-site-data: an injected failure" >&2; exit 13; }
rm -rf out/site.new; mkdir -p out/site.new
# one input every list names (stage A's output), one no list names (a sourced library),
# one outside every key (STUB_SALT, which only the shadow scenario sets)
printf 'page(%s|%s%s)\n' "$(cat out/a.txt)" "$(cat lib/helper.sh)" "${STUB_SALT:-}" > out/site.new/page.txt
git ls-files | LC_ALL=C sort > out/site.new/links.txt
rm -rf out/site; mv out/site.new out/site
printf 'status(%s)\n' "$(cat out/a.txt)" > docs/STATUS.md
SH
cat > "$BASE/scripts/gitattributes" <<'SH'
#!/usr/bin/env bash
set -eu
cd "$(dirname "$0")/.."
printf 'gitattributes\n' >> "$STUB_LOG"
printf 'out/** merge=derived\n' > .gitattributes
SH
chmod +x "$TOOL/majordomus" "$BASE/scripts/generate-site-data" "$BASE/scripts/gitattributes"
STUB_LOG="$LOG"; REAL_BIN="$RB"; export STUB_LOG REAL_BIN
export MAJORDOMUS_DERIVE_LOCK="$T/derive.lock" MAJORDOMUS_DERIVE_PRIORITY="$T/derive.priority"
unset MAJORDOMUS_DERIVE_MODE MAJORDOMUS_DERIVE_ALLOW_UNTRACKED
# the base is committed derived, as a branch is: stages C and D write files stage B reads, and
# on a tree that is not yet derived they move them after stage B ran — one more run, not a skip
( cd "$BASE" && git init -q . && git config user.email t@example.com && git config user.name t \
  && git add -A && git commit -qm sources ) || fail "the fixture could not be committed"
MAJORDOMUS_BIN="$TOOL/majordomus" bash "$BASE/scripts/derive" --full 2> "$T/base.err" || { cat "$T/base.err"; fail "the base did not derive"; }
( cd "$BASE" && git add -A && git commit -qm derived ) || fail "the derived base could not be committed"
for side in I F; do git clone -q "$BASE" "$T/$side"; ( cd "$T/$side" && git config user.email t@example.com && git config user.name t ); done
on() { local side="$1"; shift; MAJORDOMUS_BIN="$TOOL/majordomus" bash "$T/$side/scripts/derive" "$@"; }
both() { ( cd "$T/I" && eval "$1" ) && ( cd "$T/F" && eval "$1" ); }
# derive both sides the same way once: the incremental side runs (no record) and records
derive_pair() { # derive_pair <what> <ran|skipped>
  : > "$LOG"
  expect_exit 0 on I --incremental || fail "$1: the incremental derive failed"
  local ran=skipped; grep -qx site-data "$LOG" && ran=ran
  [ "$ran" = "$2" ] || fail "$1: the incremental derive ${ran} stage B, and it should have been ${2}: $(tr '\n' ' ' < "$LOG")
      it said: $(printf '%s\n' "$LAST_OUT" | grep 'stage B:' | tr '\n' ' ')"
  if [ "$2" = skipped ]; then expect_grep 'stage B: skipped \(key [0-9a-f]{12}'; fi
  expect_exit 0 on F --full || fail "$1: the full derive failed"
  if ! diff -r -x .git "$T/I" "$T/F" > "$T/diff.out" 2>&1; then
    printf '    %s: the incremental tree differs from the full one:\n' "$1"; sed 's/^/      /' "$T/diff.out" | head -20; exit 1
  fi
}
derive_pair 'the first derive (no record yet)' ran
[ -z "$(cd "$T/I" && git status --porcelain --untracked-files=all)" ] || fail "a derive of a derived tree changed it"
derive_pair 'a no-op' skipped
derive_pair 'a second no-op' skipped

both 'printf "v2\n" > src/model.txt'
derive_pair 'a tracked input any list would name' ran
both 'printf "helper two\n" > lib/helper.sh'
derive_pair 'a tracked input no list names' ran
both 'git add -A && git commit -qm edits'
derive_pair 'the edits committed' skipped

both 'rm out/site/page.txt'
derive_pair 'a deleted output' ran
both 'printf "by hand\n" > docs/STATUS.md'
derive_pair 'a tampered output' ran
both 'printf "new\n" > docs/new.md && git add docs/new.md'
derive_pair 'a staged new file' ran
grep -qx 'docs/new.md' "$T/I/out/site/links.txt" || fail "the staged new file did not reach the link list"
both 'printf "edited, not staged\n" > docs/new.md'
derive_pair 'an unstaged tracked change' ran
both 'git add -A && git commit -qm more'
derive_pair 'settled again' skipped
both 'printf "#[test] fn other() {}\n" > apps/majordomus-cli/tests/t.rs && git add -A && git commit -qm test-only'
derive_pair 'a test of the crate edited' skipped

# ------------------------------------------------------------------ a failed stage B records nothing
( cd "$T/I" && printf 'v3\n' > src/model.txt )
: > "$LOG"
STUB_FAIL=1 expect_exit 13 on I --incremental || fail "a failed stage B did not fail the derive with its own code"
( cd "$T/I" && printf 'v2\n' > src/model.txt )
: > "$LOG"
expect_exit 0 on I --incremental
grep -qx site-data "$LOG" || fail "after a failed stage B the next derive skipped it on a record nobody vouched for"
expect_grep 'must run: the run before this one did not finish'
: > "$LOG"; expect_exit 0 on I --incremental
grep -qx site-data "$LOG" && fail "the derive after a recovered one ran stage B again"

# ------------------------------------------------------------------ a stage B that reads an input outside the key
: > "$LOG"
STUB_SALT='+salt' expect_exit 13 on I --shadow || fail "a shadow derive did not refuse a stage B that changed what the skip would keep"
expect_grep 'SHADOW DIVERGENCE: the incremental mode would have skipped it'
expect_grep 'changed 1 output\(s\): out/site/page\.txt'
grep -q '+salt' "$T/I/out/site/page.txt" || fail "the shadow derive did not keep the full run's bytes"
[ "$(grep -c . "$LOG")" = 6 ] || fail "the shadow derive did not run every stage before refusing: $(tr '\n' ' ' < "$LOG")"
: > "$LOG"; expect_exit 0 on I --incremental
grep -qx site-data "$LOG" || fail "after a divergence the record that allowed the skip was still used"

# ------------------------------------------------------------------ the record lives in the git directory, never the tree
[ -f "$T/I/.git/majordomus/derive/site-data.json" ] || fail "no record in the checkout's git directory"
[ -z "$(cd "$T/I" && git status --porcelain --untracked-files=all -- .ai .git 2>/dev/null)" ] \
  || fail "the incremental derive left an untracked file"
( cd "$T/I" && MAJORDOMUS_BIN="$RB" MAJORDOMUS_SHARE="$T/I/share" "$RB" derive plan --format json ) > "$T/plan.json" || fail "derive plan exited non-zero"
grep -q '"state": "reuse"' "$T/plan.json" || fail "derive plan does not say reuse after a settled derive: $(head -c 400 "$T/plan.json")"

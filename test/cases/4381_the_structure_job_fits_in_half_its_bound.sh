# majordomus-covers: none
# claims: none
# The linter stops re-reading lib/rust_bin.sh for every case, and keeps what its names told it.
#
# Every case sources test/lib.sh, and scripts/ci/shell-lint runs `shellcheck -x` over every case
# from the repository root, so it followed test/lib.sh and, through its two
# `. "$ROOT/lib/rust_bin.sh"` lines, the 1942 lines of rust_bin.sh, once per case. Between
# 2026-10-12 and 10-25 rust_bin.sh grew from 429 to 1942 lines and the cases from 446 to 654:
# shell-lint went from 416 s to p90 1388 s of the structure job's 3300 s, and run 40813274272
# was killed at that bound with all 59 gates green (I4381). Both sources now say
# `source=test/lib/rust_bin.functions.sh`, a file of one stub per function and one empty
# assignment per global of rust_bin.sh; rust_bin.sh is linted on its own as part of lib/.
# A first version pointed them at /dev/null, and review found what that lost: shellcheck no
# longer knew the names were shell functions, so a case handing one to xargs, sudo or
# find -exec was no longer SC2033.
#
# What this case checks:
#   1. each source of rust_bin.sh in test/lib.sh carries the directive naming the declarations
#      file, and every source= directive there resolves from the root, where the linter runs;
#   2. the declarations file holds exactly rust_bin.sh's top-level function names and global
#      assignments, in its order: a name added, removed or renamed in rust_bin.sh and not here
#      fails, and so does a name here that rust_bin.sh no longer has (proved on a copy of
#      rust_bin.sh with one function added);
#   3. rust_bin.sh and the declarations file are both in shell-lint's own list;
#   4. a file that only sources test/lib.sh lints in at most three times the CPU of test/lib.sh
#      alone (it took nine to ten times as long while rust_bin.sh was followed);
#   5. SC2033 holds: `xargs mj_cargo_target_dir`, `sudo mj_rust_resolve` and
#      `find -exec mj_cargo_target_dir` in a file that sources test/lib.sh are each reported, and
#      with the directives pointed at /dev/null the same file is not (the review's fixture);
#   6. nothing is relaxed by not following: a read of a variable that only a function of
#      rust_bin.sh assigns is reported (SC2154), where with rust_bin.sh followed it passed;
#   7. every job of .ai/repo/ci/gates.yaml named structure or structure-<x> is run by `just
#      ci-structure` and named in clause 4 of project.an-expedited-landing-is-the-owners-word,
#      which say what judging "the whole structure job" locally is (proved on a copy of the
#      recipe that runs only structure). It reads the recipe's text; it does not run it.
# What it does not check: a function rust_bin.sh defines other than as `name() ` at the start of
# a line, a global set by export, readonly or declare, or one assigned inside a function (those
# are left undeclared on purpose, item 6); the arity of a function (a stub takes any arguments,
# so SC2119/SC2120 against rust_bin.sh's real bodies are not reported in a case); and any other
# check that would need rust_bin.sh's bodies rather than its names.
. "$ROOT/test/lib.sh"
command -v shellcheck >/dev/null 2>&1 || skip "shellcheck absent"
LIB="$ROOT/test/lib.sh"
RB="$ROOT/lib/rust_bin.sh"
DECL_REL="test/lib/rust_bin.functions.sh"
DECL="$ROOT/$DECL_REL"
expect_file "$LIB"
expect_file "$RB"
expect_file "$DECL"

# --- 1. the directive, above every source of rust_bin.sh, and every directive resolves
n=0
while IFS= read -r ln; do
  n=$((n + 1))
  prev="$(sed -n "$((ln - 1))p" "$LIB")"
  case "$prev" in
    *"# shellcheck source=$DECL_REL "*|*"# shellcheck source=$DECL_REL") ;;
    *) echo "    test/lib.sh:$ln sources lib/rust_bin.sh without \`# shellcheck source=$DECL_REL\` on the line above: every case re-reads rust_bin.sh, or loses its names"; exit 1 ;;
  esac
done < <(grep -n '^[[:space:]]*\. "\$ROOT/lib/rust_bin\.sh"' "$LIB" | cut -d: -f1)
[ "$n" -ge 1 ] || { echo "    test/lib.sh sources lib/rust_bin.sh nowhere; the reader has lost the lines it judges"; exit 1; }
while IFS= read -r src; do
  [ "$src" = /dev/null ] || [ -f "$ROOT/$src" ] \
    || { echo "    test/lib.sh says \`# shellcheck source=$src\`, which does not resolve from the repository root"; exit 1; }
done < <(sed -n 's/^[[:space:]]*# shellcheck source=\([^ ]*\).*/\1/p' "$LIB")

# --- 2. the declarations are rust_bin.sh's names, no more, no fewer, in its order
names_of_lib() { # <rust_bin.sh>: `f name` per top-level function, `v NAME` per global, in order
  sed -n -e 's/^\([A-Za-z_][A-Za-z0-9_]*\)() .*/f \1/p' \
    -e 's/^\([A-Za-z_][A-Za-z0-9_]*\)=.*/v \1/p' "$1"
}
names_of_decl() { # <declarations>: the same list, from the stub lines alone
  sed -n -e 's/^\([A-Za-z_][A-Za-z0-9_]*\)() { :; }$/f \1/p' \
    -e 's/^\([A-Za-z_][A-Za-z0-9_]*\)=$/v \1/p' "$1"
}
drift() { # <rust_bin.sh> <declarations>: exit 0 when they agree, 1 with the difference printed
  diff <(names_of_lib "$1") <(names_of_decl "$2")
}
names_of_lib "$RB" > "$T/lib.names"
[ "$(grep -c '^f ' "$T/lib.names")" -ge 1 ] && [ "$(grep -c '^v ' "$T/lib.names")" -ge 1 ] \
  || { echo "    found no function or no global in lib/rust_bin.sh; the reader is stale"; exit 1; }
expect_exit 0 drift "$RB" "$DECL" \
  || { echo "    $DECL_REL does not declare what lib/rust_bin.sh defines (< rust_bin.sh, > declarations); rewrite it with the command in its header"; exit 1; }
# the mutant: rust_bin.sh with one function more, beside the same declarations, is drift
{ cat "$RB"; printf 'mj_rust_added_by_4381() {\n  :\n}\n'; } > "$T/rust_bin.added.sh"
expect_exit 1 drift "$T/rust_bin.added.sh" "$DECL"
expect_grep '^< f mj_rust_added_by_4381$'

# --- 3. rust_bin.sh and the declarations are linted on their own
expect_exit 0 "$ROOT/scripts/ci/shell-lint" --list
for f in lib/rust_bin.sh "$DECL_REL"; do
  printf '%s\n' "$LAST_OUT" | grep -qx "$f" \
    || { echo "    scripts/ci/shell-lint no longer lists $f, and nothing else lints it"; exit 1; }
done

# --- 4. the cost: a file that only sources test/lib.sh, against test/lib.sh alone. CPU seconds
#     (user + system), not wall time, so that a machine other sessions load does not decide it;
#     the two are measured in turn, five times, and each keeps its least. On this tree the ratio
#     is about 1.2; with rust_bin.sh followed it was 9 to 11.
printf '#!/usr/bin/env bash\n. "$ROOT/test/lib.sh"\n' > "$T/sources-lib.sh"
lint_once() { # <file> <follow|alone>: shellcheck as shell-lint runs it, from the root
  if [ "$2" = follow ]; then (cd "$ROOT" && shellcheck -x -s bash -S warning "$1")
  else (cd "$ROOT" && shellcheck -s bash -S warning "$1"); fi
}
cpu_ms() { # <file> <follow|alone>: the CPU milliseconds of one lint
  local t
  t="$( { TIMEFORMAT='%U %S'; time lint_once "$1" "$2" >/dev/null 2>&1 || true; } 2>&1 )"
  printf '%s' "$t" | awk '{ printf "%d", ($1 + $2) * 1000 }'
}
alone=""; chained=""
for i in 1 2 3 4 5; do
  a="$(cpu_ms "$LIB" alone)"; c="$(cpu_ms "$T/sources-lib.sh" follow)"
  { [ -z "$alone" ] || [ "$a" -lt "$alone" ]; } && alone="$a"
  { [ -z "$chained" ] || [ "$c" -lt "$chained" ]; } && chained="$c"
done
[ "$alone" -gt 0 ] || alone=1
[ "$chained" -le $((alone * 3)) ] \
  || { echo "    a file that only sources test/lib.sh took ${chained} ms of CPU to lint, over three times test/lib.sh alone (${alone} ms): shellcheck follows a large file once per case again"; exit 1; }

lint() { (cd "$1" && shellcheck -x -s bash -S warning -f gcc "$2"); }
# a tree of the same lib/ and the declarations, with the test/lib.sh given on stdin
mutant_tree() { # <dir>
  mkdir -p "$1/test/lib" "$1/lib"
  cp "$ROOT"/lib/*.sh "$1/lib/"
  cp "$DECL" "$1/$DECL_REL"
  cat > "$1/test/lib.sh"
}

# --- 5. SC2033: a function of rust_bin.sh handed to an external command is reported
for fn in mj_cargo_target_dir mj_rust_resolve; do
  grep -qx "f $fn" "$T/lib.names" \
    || { echo "    lib/rust_bin.sh no longer defines $fn, which the SC2033 fixture hands to xargs, sudo and find"; exit 1; }
done
cat > "$T/hands-a-function.sh" <<'EOF'
#!/usr/bin/env bash
. "$ROOT/test/lib.sh"
printf '%s\n' apps/majordomus-cli | xargs mj_cargo_target_dir
sudo mj_rust_resolve "$ROOT" '    '
find "$ROOT/apps" -maxdepth 0 -exec mj_cargo_target_dir {} \;
EOF
expect_exit 1 lint "$ROOT" "$T/hands-a-function.sh"
for at in 3 4 5; do
  expect_grep "hands-a-function\.sh:$at:[0-9]+: warning: Shell functions can't be passed to external commands.* \[SC2033\]" \
    || { echo "    line $at of the fixture hands a function of rust_bin.sh to an external command, and shellcheck did not say so"; exit 1; }
done
# the mutant: the directives pointed at /dev/null, as the first version of I4381 had them
sed 's|# shellcheck source=test/lib/rust_bin\.functions\.sh # I4381|# shellcheck source=/dev/null # I4381|' "$LIB" \
  | mutant_tree "$T/devnull"
grep -q 'source=/dev/null # I4381' "$T/devnull/test/lib.sh" || { echo "    the /dev/null mutation did not take"; exit 1; }
expect_exit 0 lint "$T/devnull" "$T/hands-a-function.sh"
expect_no_grep 'SC2033'

# --- 6. nothing is relaxed: a variable only a function of rust_bin.sh assigns is reported when a
#     case reads it. The variable is read from rust_bin.sh, never named here, and proved to be
#     one the followed file would have hidden, so the case cannot pass on a name that is
#     reported for another reason.
var="$(sed -n 's/^  \(mj_[a-z0-9_]*\)=.*/\1/p' "$RB" | while IFS= read -r v; do
  grep -q "$v" "$LIB" || grep -q "$v" "$DECL" || { printf '%s\n' "$v"; break; }
done)"
[ -n "$var" ] || { echo "    found no function-level variable of lib/rust_bin.sh to read; the reader is stale"; exit 1; }
printf '#!/usr/bin/env bash\n. "$ROOT/test/lib.sh"\nprintf "%%s\\n" "$%s"\n' "$var" > "$T/reads-rust-bin.sh"
expect_exit 1 lint "$ROOT" "$T/reads-rust-bin.sh"
expect_grep "$var is referenced but not assigned. \[SC2154\]"
# the mutant: test/lib.sh without the directives, beside the same lib/, follows rust_bin.sh, and
# the same file passes
grep -v '# shellcheck source=test/lib/rust_bin\.functions\.sh # I4381' "$LIB" | mutant_tree "$T/mutant"
grep -q 'rust_bin\.functions\.sh # I4381' "$T/mutant/test/lib.sh" && { echo "    the mutation did not take"; exit 1; }
expect_exit 0 lint "$T/mutant" "$T/reads-rust-bin.sh"

# --- 7. "the whole structure job" is every structure job of the model, locally and in the rule
CIJ="$ROOT/.just/ci.just"
RULE="$ROOT/.ai/repo/rules/project/an-expedited-landing-is-the-owners-word.v1.md"
expect_file "$CIJ"
expect_file "$RULE"
sed -n 's/^[[:space:]]*job:[[:space:]]*\(structure\(-[a-z0-9]*\)\{0,1\}\)[[:space:]]*$/\1/p' \
  "$ROOT/.ai/repo/ci/gates.yaml" | sort -u > "$T/structure.jobs"
grep -qx structure "$T/structure.jobs" && [ "$(wc -l < "$T/structure.jobs")" -ge 2 ] \
  || { echo "    .ai/repo/ci/gates.yaml assigns gates to fewer than two structure jobs; the reader is stale"; exit 1; }
recipe_body() { # <ci.just>: the lines of the ci-structure recipe
  awk '/^ci-structure:/ { on = 1; next } on && /^[^ ]/ { exit } on' "$1"
}
recipe_runs_every_job() { # <ci.just>: exit 1 naming each structure job the recipe does not run
  local body j miss=0
  body="$(recipe_body "$1")"
  [ -n "$body" ] || { echo "no ci-structure recipe"; return 1; }
  while IFS= read -r j; do
    printf '%s\n' "$body" | grep -qE "(^|[ \"'])$j([ \"';]|\$)" || { echo "does not run $j"; miss=1; }
  done < "$T/structure.jobs"
  return "$miss"
}
expect_exit 0 recipe_runs_every_job "$CIJ" \
  || { echo "    just ci-structure does not run every structure job of the model, so a local judgement of the structure job leaves gates out"; exit 1; }
clause4="$(awk '/^4\. \*\*/ { on = 1 } on && /^5\. \*\*/ { exit } on' "$RULE")"
while IFS= read -r j; do
  printf '%s\n' "$clause4" | grep -qF "\`$j\`" \
    || { echo "    clause 4 of $(basename "$RULE") does not name the structure job $j, so a lander who follows it leaves its gates unjudged"; exit 1; }
done < "$T/structure.jobs"
# the mutant: the recipe as it was, running only the job structure
sed 's/for job in structure structure-b; do/for job in structure; do/' "$CIJ" > "$T/ci.only-structure.just"
grep -q 'for job in structure; do' "$T/ci.only-structure.just" || { echo "    the recipe mutation did not take"; exit 1; }
expect_exit 1 recipe_runs_every_job "$T/ci.only-structure.just"
expect_grep 'does not run structure-b'

echo "    shellcheck reads test/lib.sh's chain through $DECL_REL, not lib/rust_bin.sh (${chained} ms against ${alone} ms alone); its names agree with rust_bin.sh's; SC2033 holds on xargs, sudo and find -exec; a read of a variable only rust_bin.sh assigns ($var) is still reported; and just ci-structure and the rule name every structure job ($(tr '\n' ' ' < "$T/structure.jobs" | sed 's/ $//'))"

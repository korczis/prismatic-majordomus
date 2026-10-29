# majordomus-covers: none
# claims: none
# The linter stops re-reading lib/rust_bin.sh for every case, and no finding is lost by it.
#
# Every case sources test/lib.sh, and scripts/ci/shell-lint runs `shellcheck -x` over every case
# from the repository root, so it followed test/lib.sh and, through its two
# `. "$ROOT/lib/rust_bin.sh"` lines, the 1942 lines of rust_bin.sh, once per case. Between
# 2026-10-12 and 10-25 rust_bin.sh grew from 429 to 1942 lines and the cases from 446 to 654:
# shell-lint went from 416 s to p90 1388 s of the structure job's 3300 s, and run 40813274272
# was killed at that bound with all 59 gates green (I4381). Both sources now say
# `source=/dev/null`; rust_bin.sh is linted on its own as part of lib/.
#
# The case holds four things: each source of rust_bin.sh in test/lib.sh carries the directive,
# and every source= directive there resolves from the root, where the linter runs; rust_bin.sh
# is still in shell-lint's own list; a file that only sources test/lib.sh lints in at most three
# times what test/lib.sh alone takes (it took nine to ten times as long while the chain was
# followed); and the /dev/null source relaxes nothing: a case that reads a variable only
# rust_bin.sh assigns is reported (SC2154), where with the chain followed it passed.
. "$ROOT/test/lib.sh"
command -v shellcheck >/dev/null 2>&1 || skip "shellcheck absent"
LIB="$ROOT/test/lib.sh"
expect_file "$LIB"

# --- 1. the directive, above every source of rust_bin.sh, and every directive resolves
n=0
while IFS= read -r ln; do
  n=$((n + 1))
  prev="$(sed -n "$((ln - 1))p" "$LIB")"
  case "$prev" in
    *'# shellcheck source=/dev/null'*) ;;
    *) echo "    test/lib.sh:$ln sources lib/rust_bin.sh without \`# shellcheck source=/dev/null\` on the line above: every case re-reads it"; exit 1 ;;
  esac
done < <(grep -n '^[[:space:]]*\. "\$ROOT/lib/rust_bin\.sh"' "$LIB" | cut -d: -f1)
[ "$n" -ge 1 ] || { echo "    test/lib.sh sources lib/rust_bin.sh nowhere; the reader has lost the lines it judges"; exit 1; }
while IFS= read -r src; do
  [ "$src" = /dev/null ] || [ -f "$ROOT/$src" ] \
    || { echo "    test/lib.sh says \`# shellcheck source=$src\`, which does not resolve from the repository root"; exit 1; }
done < <(sed -n 's/^[[:space:]]*# shellcheck source=\([^ ]*\).*/\1/p' "$LIB")

# --- 2. rust_bin.sh is linted on its own
expect_exit 0 "$ROOT/scripts/ci/shell-lint" --list
printf '%s\n' "$LAST_OUT" | grep -qx 'lib/rust_bin.sh' \
  || { echo "    scripts/ci/shell-lint no longer lists lib/rust_bin.sh, and nothing else lints it"; exit 1; }

# --- 3. the cost: a file that only sources test/lib.sh, against test/lib.sh alone. CPU seconds
#     (user + system), not wall time, so that a machine other sessions load does not decide it;
#     the two are measured in turn, five times, and each keeps its least. On this tree the ratio
#     is about 1.2; with the chain followed it was 9 to 11.
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

# --- 4. nothing is relaxed: a variable only rust_bin.sh assigns is reported when a case reads it.
#     The variable is read from rust_bin.sh, never named here, and proved to be one the followed
#     chain would have hidden, so the case cannot pass on a name that is reported for another reason.
var="$(sed -n 's/^  \(mj_[a-z0-9_]*\)=.*/\1/p' "$ROOT/lib/rust_bin.sh" | while IFS= read -r v; do
  grep -q "$v" "$LIB" || { printf '%s\n' "$v"; break; }
done)"
[ -n "$var" ] || { echo "    found no function-level variable of lib/rust_bin.sh to read; the reader is stale"; exit 1; }
printf '#!/usr/bin/env bash\n. "$ROOT/test/lib.sh"\nprintf "%%s\\n" "$%s"\n' "$var" > "$T/reads-rust-bin.sh"
lint() { (cd "$1" && shellcheck -x -s bash -S warning -f gcc "$2"); }
expect_exit 1 lint "$ROOT" "$T/reads-rust-bin.sh"
expect_grep "$var is referenced but not assigned. \[SC2154\]"
# the mutant: test/lib.sh without the directive, beside the same lib/, lets the same file pass
mkdir -p "$T/mutant/test" "$T/mutant/lib"
cp "$ROOT"/lib/*.sh "$T/mutant/lib/"
grep -v '# shellcheck source=/dev/null # I4381' "$LIB" > "$T/mutant/test/lib.sh"
grep -q 'source=/dev/null # I4381' "$T/mutant/test/lib.sh" && { echo "    the mutation did not take"; exit 1; }
expect_exit 0 lint "$T/mutant" "$T/reads-rust-bin.sh"

echo "    shellcheck reads test/lib.sh's chain without lib/rust_bin.sh (${chained} ms against ${alone} ms alone), and a read of a variable only rust_bin.sh assigns ($var) is still reported"

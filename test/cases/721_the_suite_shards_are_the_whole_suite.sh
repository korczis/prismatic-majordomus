# majordomus-covers: none
# The suite runs as shards, and the shards are the whole suite: every case exactly once.
#
# One runner took more than two hours: 24,879 case-seconds dealt to four workers. Dealt to
# sixteen workers on four runners (MJ_TEST_SHARD=i/4), the critical path is the longest
# single case. The risk sharding brings is the one this case exists for — a case dealt to no
# shard, or to two, reads as a quieter or a slower suite rather than a wrong one. So:
#
#   1. the union of the four shards' cases is every case under test/cases/, none twice
#   2. the deal is a function of the committed durations: dealing twice deals the same hand
#   3. an unknown case (no recorded seconds) is still dealt, to exactly one shard
#   4. `--verify-report` passes a complete report and refuses a missing, a repeated and an
#      unknown row
#   5. the workflow runs the shards and the suite job proves their union before its verdict
. "$ROOT/test/lib.sh"

all="$T/all.txt"; union="$T/union.txt"
for c in "$ROOT"/test/cases/*.sh; do basename "$c" .sh; done | LC_ALL=C sort > "$all"
deal() {   # <shard> [durations file]
  env MJ_TEST_JOBS=4 MJ_TEST_SHARD="$1/4" MJ_TEST_LIST=1 \
    MJ_TEST_DURATIONS="${2:-$ROOT/.ai/repo/ci/suite-durations.tsv}" bash "$ROOT/test/run.sh"
}

# ---------------------------------------------------------------- 1. the union is the suite
: > "$union"
for i in 1 2 3 4; do deal "$i" > "$T/shard$i.txt"; cat "$T/shard$i.txt" >> "$union"; done
dups="$(LC_ALL=C sort "$union" | uniq -d)"
[ -z "$dups" ] || { echo "    a case was dealt to two shards: $dups"; exit 1; }
LC_ALL=C sort "$union" | cmp -s - "$all" || {
  echo "    the shards are not the suite:"; LC_ALL=C sort "$union" | diff - "$all" | head; exit 1; }
for i in 1 2 3 4; do [ -s "$T/shard$i.txt" ] || { echo "    shard $i was dealt nothing"; exit 1; }; done

# ---------------------------------------------------------------- 1b. the heaviest open apart
# Four 35-minute cases dealt to one runner's four workers starved each other past their
# timeout (#701's first run). The four heaviest parallel cases go to four different shards.
heavy="$(LC_ALL=C sort -t "$(printf '\t')" -k2,2nr "$ROOT/.ai/repo/ci/suite-durations.tsv" |
  while IFS="$(printf '\t')" read -r name _; do
    f="$ROOT/test/cases/$name.sh"
    [ -f "$f" ] && ! grep -q '^# majordomus-exclusive:' "$f" && echo "$name"
  done | head -4)"
[ "$(printf '%s\n' "$heavy" | grep -c .)" = 4 ] || { echo "    fewer than four timed parallel cases"; exit 1; }
seen=""
for name in $heavy; do
  s="$(grep -lx "$name" "$T"/shard[1-4].txt)"
  case " $seen " in *" $s "*) echo "    two of the heaviest cases share $(basename "$s" .txt): $heavy"; exit 1 ;; esac
  seen="$seen $s"
done

# ---------------------------------------------------------------- 2. the deal is deterministic
for i in 1 2 3 4; do
  deal "$i" | cmp -s - "$T/shard$i.txt" || { echo "    dealing shard $i twice dealt two hands"; exit 1; }
done

# ---------------------------------------------------------------- 3. an unknown case is dealt
# durations that know only half the cases: the rest weigh the default and are still dealt
half="$T/half.tsv"
awk 'NR % 2' "$ROOT/.ai/repo/ci/suite-durations.tsv" > "$half"
: > "$union"
for i in 1 2 3 4; do deal "$i" "$half" >> "$union"; done
LC_ALL=C sort "$union" | cmp -s - "$all" || { echo "    with half the durations unknown, the shards are not the suite"; exit 1; }
: > "$union"
for i in 1 2 3 4; do deal "$i" /nonexistent >> "$union"; done
LC_ALL=C sort "$union" | cmp -s - "$all" || { echo "    with no durations file, the shards are not the suite"; exit 1; }

# ---------------------------------------------------------------- 4. the report is verified
awk -v OFS='\t' '{print $1, "ok", 1, "parallel"}' "$all" > "$T/full.tsv"
expect_exit 0 bash "$ROOT/test/run.sh" --verify-report "$T/full.tsv"
sed 1d "$T/full.tsv" > "$T/missing.tsv"
expect_exit 1 bash "$ROOT/test/run.sh" --verify-report "$T/missing.tsv"
expect_grep "no row for"
{ cat "$T/full.tsv"; head -1 "$T/full.tsv"; } > "$T/twice.tsv"
expect_exit 1 bash "$ROOT/test/run.sh" --verify-report "$T/twice.tsv"
expect_grep "more than one row"
{ cat "$T/full.tsv"; printf 'not_a_case\tok\t1\tparallel\n'; } > "$T/unknown.tsv"
expect_exit 1 bash "$ROOT/test/run.sh" --verify-report "$T/unknown.tsv"
expect_grep "rows for no case"

# ---------------------------------------------------------------- 4b. a case never inherits the deal
# CI exports MJ_TEST_SHARD to the whole step; a case that runs test/run.sh itself must not
# find it. The runner unsets it for every case, which a nested probe proves.
probe="$ROOT/test/cases/00_yaml_flatten.sh"
[ -f "$probe" ] || { echo "    the probe case is missing"; exit 1; }
grep -q 'MJ_TEST_SHARD MJ_TEST_DURATIONS MJ_TEST_LIST' "$ROOT/test/run.sh" \
  || { echo "    test/run.sh does not unset the shard selection before running a case"; exit 1; }

# ---------------------------------------------------------------- 5. the workflow is wired
W="$ROOT/.github/workflows/validate.yml"
job() { awk -v j="  $1:" '$0 == j {f=1; next} /^  [a-z-]+:$/ {f=0} f' "$W"; }
job suite-shard | grep -q 'MJ_TEST_SHARD: \${{ matrix.shard }}/4' || { echo "    the suite-shard job does not run its shard"; exit 1; }
job suite-shard | grep -q 'shard: \[1, 2, 3, 4\]' || { echo "    the suite-shard matrix is not the four shards the deal assumes"; exit 1; }
job suite | grep -q 'test/run.sh --verify-report suite.tsv' || { echo "    the suite job does not prove the shards' union"; exit 1; }
job suite | grep -q 'needs.suite-shard.result' || { echo "    the suite job does not fail when a shard failed"; exit 1; }
exit 0

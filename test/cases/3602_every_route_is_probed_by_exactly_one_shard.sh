# majordomus-covers: none
# The site's browser probe runs as N shards side by side (I2841). Splitting it must not drop a
# route or measure one twice: before the split one step probed every route at every width for
# 35-37 minutes, and a sweep that silently skips a page is the failure the full sweep exists to
# prevent (every overflow this site has had came from canonical data in a shared template).
#
#   partition   for route lists of 1 to 13 routes and 1 to 6 shards, the shards that
#               `site-probe --partition K/N` gives are disjoint and their union is the list
#   balanced    no shard holds more than one route more than another
#   refused     a malformed or out-of-range K/N is refused (exit 2), and --quick refuses a shard
#   wired       the workflow's site-probe matrix runs shards 1..N with no gap and hands each one
#               SITE_PROBE_SHARD=<shard>/N for that same N, so the jobs together are the sweep
#
# A mutation of shard_of that keeps every route in every shard, or that drops the last stride,
# turns `partition` red; a matrix that loses a shard turns `wired` red.
. "$ROOT/test/lib.sh"
P="$ROOT/scripts/site-probe"

fail=0
for size in 1 2 3 4 5 6 7 8 9 10 11 12 13; do
  seq 1 "$size" | sed 's#.*#/route-&/#' > list
  for n in 1 2 3 4 5 6; do
    [ "$n" -le "$size" ] || continue
    : > union
    min=999999; max=0
    for k in $(seq 1 "$n"); do
      "$P" --partition "$k/$n" < list > "shard.$k" || { echo "    FAIL: --partition $k/$n exited $?"; fail=1; }
      c="$(wc -l < "shard.$k" | tr -d ' ')"
      [ "$c" -lt "$min" ] && min=$c; [ "$c" -gt "$max" ] && max=$c
      cat "shard.$k" >> union
    done
    if ! diff -q <(LC_ALL=C sort list) <(LC_ALL=C sort union) >/dev/null; then
      echo "    FAIL: $size route(s) in $n shard(s): the union is not the list"
      diff <(LC_ALL=C sort list) <(LC_ALL=C sort union) | sed 's/^/    | /' | head -6; fail=1
    fi
    dup="$(sort union | uniq -d | head -1)"
    [ -z "$dup" ] || { echo "    FAIL: $size route(s) in $n shard(s): $dup is in more than one shard"; fail=1; }
    [ $((max - min)) -le 1 ] || { echo "    FAIL: $size route(s) in $n shard(s): shards hold $min to $max"; fail=1; }
  done
done
[ "$fail" = 0 ] || exit 1
echo "    ok: every route list of 1-13 routes splits into 1-6 disjoint shards whose union is the list"
echo "    ok: no shard holds more than one route more than another"

for bad in 0/3 4/3 3 x/2 2/x /2 ''; do
  rc=0; "$P" --partition "$bad" < list > /dev/null 2>&1 || rc=$?
  [ "$rc" = 2 ] || { echo "    FAIL: --partition '$bad' exited $rc, expected 2"; exit 1; }
done
rc=0; SITE_PROBE_SHARD=2/4 "$P" --quick > out 2>&1 || rc=$?
[ "$rc" = 2 ] && grep -q 'cannot be split' out || { echo "    FAIL: --quick with a shard exited $rc"; cat out; exit 1; }
rc=0; SITE_PROBE_SHARD=5/4 "$P" > out 2>&1 || rc=$?
[ "$rc" = 2 ] || { echo "    FAIL: SITE_PROBE_SHARD=5/4 exited $rc, expected 2"; cat out; exit 1; }
echo "    ok: a malformed or out-of-range shard is refused, and --quick refuses to be split"

# The fan-in: shard reports that are the whole sweep pass; a dropped route, a route measured
# twice and a missing shard are each refused, naming what is wrong.
seq 1 7 | sed 's#.*#/r&/#' > list7
report() { # $1 dir, $2 K, $3 N
  mkdir -p "$1/shard-$2"
  { printf '# shard %s/%s of 7 routes\n' "$2" "$3"; "$P" --partition "$2/$3" < list7; } > "$1/shard-$2/site-probe-routes.txt"
}
for k in 1 2 3; do report whole "$k" 3; done
"$P" --verify-report whole > out 2>&1 || { echo "    FAIL: three shards that are the sweep were refused"; cat out; exit 1; }
cp -R whole dropped; sed -i.bak '$d' dropped/shard-3/site-probe-routes.txt; rm -f dropped/shard-3/*.bak
rc=0; "$P" --verify-report dropped > out 2>&1 || rc=$?
[ "$rc" = 10 ] && grep -q 'measured 6 route(s) of the 7' out || { echo "    FAIL: a dropped route was not refused (exit $rc)"; cat out; exit 1; }
cp -R whole twice; head -2 twice/shard-1/site-probe-routes.txt | tail -1 >> twice/shard-2/site-probe-routes.txt
rc=0; "$P" --verify-report twice > out 2>&1 || rc=$?
[ "$rc" = 10 ] && grep -q 'more than one shard' out || { echo "    FAIL: a route measured twice was not refused (exit $rc)"; cat out; exit 1; }
cp -R whole missing; rm -r missing/shard-2
rc=0; "$P" --verify-report missing > out 2>&1 || rc=$?
[ "$rc" = 10 ] && grep -q 'not each of 1..3 once' out || { echo "    FAIL: a missing shard was not refused (exit $rc)"; cat out; exit 1; }
echo "    ok: the fan-in accepts the whole sweep and refuses a dropped route, a duplicate and a missing shard"

# The workflow: the matrix of the site-probe job and the N each job is told.
WF="$ROOT/.github/workflows/validate.yml"
job_of() { awk -v j="  $1:" '$0 == j {f=1; next} f && /^  [a-z][a-z-]*:$/{exit} f' "$WF"; }
job="$(job_of site-probe-shard)"
[ -n "$job" ] || { echo "    FAIL: validate.yml has no site-probe-shard job"; exit 1; }
shards="$(printf '%s\n' "$job" | sed -n 's/^ *shard: *\[\(.*\)\] *$/\1/p' | tr -d ' ')"
[ -n "$shards" ] || { echo "    FAIL: the site-probe-shard job declares no shard matrix"; exit 1; }
n="$(printf '%s\n' "$shards" | tr ',' '\n' | grep -c .)"
want="$(seq 1 "$n" | paste -sd, -)"
[ "$shards" = "$want" ] || { echo "    FAIL: the site-probe-shard matrix is [$shards], not [$want]"; exit 1; }
printf '%s\n' "$job" | grep -qF "SITE_PROBE_SHARD: \${{ matrix.shard }}/$n" \
  || { echo "    FAIL: the site-probe-shard job does not hand each shard SITE_PROBE_SHARD=<shard>/$n"; exit 1; }
fanin="$(job_of site-probe)"
printf '%s\n' "$fanin" | grep -q 'scripts/site-probe --verify-report' \
  || { echo "    FAIL: the site-probe job does not verify the shards' reports before the gate's receipt"; exit 1; }
! job_of site | grep -q '^ *run:.*scripts/site-probe' \
  || { echo "    FAIL: the site job still runs the whole probe beside the shards"; exit 1; }
echo "    ok: the workflow runs shards 1..$n, each told SITE_PROBE_SHARD=<shard>/$n; the fan-in verifies them; the site job no longer probes"

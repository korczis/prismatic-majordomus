# majordomus-covers: none
# An answer says when the state behind it was read, and the Cockpit shows that time.
#
# The index and the git state are pictures a long-lived server takes and re-takes when the
# repository moves (crate::live). Before the freshness contract nothing said how old a
# picture was: the Overview counted objects and read "clean" with no as-of time, and a
# reader could not tell a picture from a minute ago from one from this morning.
#
#   1. repository.info over HTTP carries repository.observed.index and .git, each an RFC 3339
#      observed_at, a source, and a stale_after after it; health.report carries the same
#      observations for the checks that judged those pictures;
#   2. the Cockpit Overview shows the index's and the git read's observed_at, and the Health
#      page shows the git read's beside the Version control verdict;
#   3. after a commit the next answer carries a later observed_at and the new head, and the
#      pages show the new time and no longer the old one: refreshed, not silently frozen.
#
# Mutation-proven: a fixed observation time fails step 3, and the Overview without its
# as-of line fails step 2.
. "$ROOT/test/lib.sh"
command -v curl >/dev/null 2>&1 || skip "no curl"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj545.XXXXXX")"
SRV=""; trap 'rm -rf "$S"; [ -n "$SRV" ] && kill "$SRV" 2>/dev/null' EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm layer
serve_up "$S/out.txt" "$S/err.txt" || exit 1
get() { curl -s -m 20 "$U$1"; }

RFC='^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$'

# ---------------------------------------------------------------- 1. the answers carry it
get /api/v1/repository > "$S/r1.json"
for p in index git; do
  jq -e --arg p "$p" --arg rfc "$RFC" '
    .repository.observed[$p] as $o
    | ($o.observed_at | test($rfc)) and ($o.stale_after | test($rfc))
      and ($o.stale_after > $o.observed_at) and ($o.source | length > 0)' "$S/r1.json" >/dev/null \
    || { echo "    repository.info carries no usable $p observation"; jq -c '.repository.observed' "$S/r1.json"; exit 1; }
done
I1="$(jq -r '.repository.observed.index.observed_at' "$S/r1.json")"
G1="$(jq -r '.repository.observed.git.observed_at' "$S/r1.json")"
H1="$(jq -r '.repository.git.head' "$S/r1.json")"
jq -e '.repository.observed.index.source == "index build" and .repository.observed.git.source == "git read"' \
  "$S/r1.json" >/dev/null || { echo "    the observations do not name what was read"; exit 1; }

get /api/v1/health > "$S/h1.json"
jq -e --arg g "$G1" --arg i "$I1" '.observed.git.observed_at == $g and .observed.layer.observed_at == $i' \
  "$S/h1.json" >/dev/null \
  || { echo "    health.report does not carry the observations of the pictures its checks judged"; jq -c '.observed' "$S/h1.json"; exit 1; }

# ---------------------------------------------------------------- 2. the Cockpit shows it
get /cockpit > "$S/o1.html"
grep -q "data-observed-at=\"$I1\"" "$S/o1.html" \
  || { echo "    the Overview does not show when the index was built ($I1)"; exit 1; }
grep -q "index build as of $I1" "$S/o1.html" \
  || { echo "    the Overview's statistics carry no as-of line"; exit 1; }
grep -q "git read as of $G1" "$S/o1.html" \
  || { echo "    the Overview's Version control fact carries no as-of time"; exit 1; }
get /cockpit/health > "$S/p1.html"
grep -q "git read as of $G1" "$S/p1.html" \
  || { echo "    the Health page's Version control card carries no as-of time"; exit 1; }
grep -q 'once at startup' "$S/p1.html" \
  && { echo "    the Health page still says git was asked once at startup"; exit 1; }

# ---------------------------------------------------------------- 3. after a change: refreshed
# RFC 3339 in whole seconds: wait until the clock has moved past the first observation
sleep 1.2
echo "moved" > moved.txt
git add moved.txt && git commit -qm "a change while the server runs"
H2="$(git rev-parse HEAD)"

get /api/v1/repository > "$S/r2.json"
I2="$(jq -r '.repository.observed.index.observed_at' "$S/r2.json")"
G2="$(jq -r '.repository.observed.git.observed_at' "$S/r2.json")"
[ "$(jq -r '.repository.git.head' "$S/r2.json")" = "$H2" ] \
  || { echo "    the server did not re-read the repository after the commit (head $H1 still)"; exit 1; }
[[ "$I2" > "$I1" ]] || { echo "    the index observation did not move with the new picture: $I1 -> $I2"; exit 1; }
[[ "$G2" > "$G1" ]] || { echo "    the git observation did not move with the new picture: $G1 -> $G2"; exit 1; }

get /cockpit > "$S/o2.html"
grep -q "index build as of $I2" "$S/o2.html" \
  || { echo "    the Overview does not show the new picture's time ($I2)"; exit 1; }
grep -q "index build as of $I1" "$S/o2.html" \
  && { echo "    the Overview still shows the old picture's time ($I1)"; exit 1; }
grep -q "${H2:0:12}" "$S/o2.html" \
  || { echo "    the Overview's Version control fact does not show the new head"; exit 1; }
# health.report is cached for five seconds under a key the commit above does not change (the
# registry fingerprint), so for up to that long the page may show the previous picture. What
# it must never do is show that picture as current: whatever time it shows is the time its
# answer carries, and once the entry has expired it is the new one.
i=0
while :; do
  get /cockpit/health > "$S/p2.html"
  get /api/v1/health > "$S/h2.json"
  grep -q "git read as of $G2" "$S/p2.html" && break
  SHOWN="$(grep -o 'git read as of [0-9TZ:-]*' "$S/p2.html" | head -n 1 | sed 's/git read as of //')"
  [ -n "$SHOWN" ] && [[ ! "$SHOWN" > "$G2" ]] \
    || { echo "    the Health page shows a git read ($SHOWN) that is neither the old one nor the new one ($G2)"; exit 1; }
  i=$((i+1))
  [ "$i" -lt 20 ] || { echo "    the Health page still shows the git read of $SHOWN, not $G2, 10 s after the commit"; exit 1; }
  sleep 0.5
done
serve_down
echo "    observed $I1 -> $I2 (index), $G1 -> $G2 (git); the Cockpit followed"

# majordomus-covers: none
# A publication that did not arrive is a finding the next reader meets, never a green step (I2661).
#
# The step that waits for the public site to name the commit is continue-on-error, so a site
# that never changes used to end the deploy job green while production served the previous
# commit. `scripts/pages publication` states that in the three places a lander looks without
# opening a log: the run's summary, an annotation, and a `not served` row of the timing table;
# and `scripts/pages verify` says, when GitHub's own build is still waiting, that it is and since
# when. The stub is `gh` (the boundary where the unknown lives); the site is a local server that
# answers a build.json naming another commit and never changes.
. "$ROOT/test/lib.sh"

command -v curl >/dev/null 2>&1 || skip "no curl"
command -v python3 >/dev/null 2>&1 || skip "no python3 to serve a site that never changes"
command -v jq >/dev/null 2>&1 || skip "no jq"

WANT=1234567890abcdef1234567890abcdef12345678   # the commit that was pushed
OLD=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa    # the commit production still serves
SINCE=2026-10-16T01:02:03Z
BIN="$T/bin"; SITE="$T/site"; mkdir -p "$BIN" "$SITE"
printf '{ "commit": "%s" }\n' "$OLD" > "$SITE/build.json"

PORT="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')"
( cd "$SITE" && exec python3 -m http.server "$PORT" --bind 127.0.0.1 >/dev/null 2>&1 ) &
SERVER=$!
trap 'kill $SERVER 2>/dev/null || true' EXIT
URL="http://127.0.0.1:$PORT"
i=0; until curl -fsS --max-time 2 "$URL/build.json" >/dev/null 2>&1; do
  i=$((i + 1)); [ "$i" -lt 30 ] || { echo "    the local site did not come up"; exit 1; }; sleep 0.2
done

TIP="$(git -C "$ROOT" rev-parse origin/gh-pages 2>/dev/null || echo 0000000000000000000000000000000000000000)"
stub_gh() {   # $1 the build status; tab-separated rows, which is what `pages built` parses
  cat > "$BIN/gh" <<STUB
#!/bin/sh
printf '%s\t%s\tno message\t%s\n' "$1" "$TIP" "$SINCE"
STUB
  chmod +x "$BIN/gh"
}

# --- 1. verify, with GitHub's build queued: it says it is queued, and since when
stub_gh queued
rc=0
PATH="$BIN:$PATH" "$ROOT/scripts/pages" verify --url "$URL" --commit "$WANT" --timeout 0 \
  > "$T/v.out" 2> "$T/v.err" || rc=$?
[ "$rc" = 12 ] || { echo "    a queued build exited $rc, not 12"; cat "$T/v.err"; exit 1; }
grep -q "queued its build" "$T/v.err" && grep -q "since $SINCE" "$T/v.err" || {
  echo "    verify does not say GitHub's build is queued and since when:"; cat "$T/v.err"; exit 1; }
echo "    a publication GitHub has queued is reported as queued, and since when"

# --- 2. the three signals of a publication that did not arrive
: > "$T/summary.md"; printf 'pages\tbuild\t10\n' > "$T/timings.tsv"
PATH="$BIN:$PATH" GITHUB_STEP_SUMMARY="$T/summary.md" "$ROOT/scripts/pages" publication \
  --commit "$WANT" --published success --served failure --url "$URL" --timings "$T/timings.tsv" \
  > "$T/p.out" 2> "$T/p.err" || { echo "    publication exited non-zero: it is a statement, never a gate"; cat "$T/p.err"; exit 1; }
grep -q 'not served' "$T/summary.md" && grep -q "$OLD" "$T/summary.md" && grep -q "queued since $SINCE" "$T/summary.md" || {
  echo "    the summary does not say not served, what is served and that GitHub is queued since when:"; cat "$T/summary.md"; exit 1; }
grep -q '^::warning title=Public site not served::' "$T/p.out" || {
  echo "    no annotation for a publication that did not arrive:"; cat "$T/p.out"; exit 1; }
awk -F'\t' '$1 == "pages" && $2 == "public" && $3 == "not served" { f = 1 } END { exit !f }' "$T/timings.tsv" || {
  echo "    the timing table has no 'not served' row:"; cat "$T/timings.tsv"; exit 1; }
echo "    summary, annotation and timing row each say the publication did not arrive"

# --- 3. the mutation: a site that does serve the commit says none of it
: > "$T/summary2.md"; printf 'pages\tbuild\t10\n' > "$T/timings2.tsv"
PATH="$BIN:$PATH" GITHUB_STEP_SUMMARY="$T/summary2.md" "$ROOT/scripts/pages" publication \
  --commit "$WANT" --published success --served success --url "$URL" --timings "$T/timings2.tsv" \
  > "$T/p2.out" 2>&1
! grep -q 'not served' "$T/summary2.md" "$T/p2.out" "$T/timings2.tsv" || {
  echo "    a publication that arrived is reported as not served"; exit 1; }
grep -q 'serves' "$T/summary2.md" || { echo "    a publication that arrived is not reported as served"; exit 1; }
[ "$(wc -l < "$T/timings2.tsv" | tr -d ' ')" = 1 ] || { echo "    a served publication added a timing row"; exit 1; }
echo "    a publication that arrived reports no finding"

# --- 4. nothing was pushed: said as an error, not as a wait
: > "$T/summary3.md"
PATH="$BIN:$PATH" GITHUB_STEP_SUMMARY="$T/summary3.md" "$ROOT/scripts/pages" publication \
  --commit "$WANT" --published failure --served skipped > "$T/p3.out" 2>&1
grep -q '^::error title=Not published::' "$T/p3.out" && grep -q 'not pushed' "$T/summary3.md" || {
  echo "    a run that pushed nothing does not say so"; cat "$T/p3.out" "$T/summary3.md"; exit 1; }
echo "    a run that pushed nothing is said to have pushed nothing"

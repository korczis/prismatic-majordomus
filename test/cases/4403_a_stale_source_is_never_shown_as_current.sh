# majordomus-covers: none
# A fact the brief holds past its bound, or one master has moved past, is never shown as
# current (I4390).
#
# On 2026-10-29 `majordomus brief` showed a 17 h old observation of the integration queue as
# today's count, while origin/master was three merges further on. The integration section
# now carries its observation: when it was made, how old it is against the bound its owner
# declares (OBSERVATION_STALE_AFTER, one hour), the master it saw against origin/master now,
# a verdict word and the remedy. The one line says `prs stale <age>`, never a bare count.
#
# What it proves, over the executable on a fixture checkout with no server:
#   1  an observation two hours old reads `stale` with the remedy `majordomus prs refresh`,
#      and the line says `prs stale 2h`
#   2  one made a minute ago at a master origin/master has moved past reads `stale` too, and
#      says by how many commits master moved
#   3  one made a minute ago at origin/master reads `current`
#   4  the check this case makes refuses an answer patched to call the stale one current
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "no jq"
export MAJORDOMUS_SHARE="$ROOT/share"
fail() { printf '    %s\n' "$*"; exit 1; }

"$MJ" init >/dev/null
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml
git add -A && git commit -qm base
base="$(git rev-parse HEAD)"
git commit -q --allow-empty -m "one more on master"
now="$(git rev-parse HEAD)"

# observe <seconds ago> <the master the forge reported>
observe() {
  at="$(python3 -c "import datetime,sys; print((datetime.datetime.now(datetime.timezone.utc)-datetime.timedelta(seconds=int(sys.argv[1]))).strftime('%Y-%m-%dT%H:%M:%SZ'))" "$1")"
  mkdir -p .ai/local/state/integration
  printf '{"schema":4,"repository":"owner/repo","base":"master","base_sha":"%s","observed_at":"%s","pull_requests":[]}\n' "$2" "$at" \
    > .ai/local/state/integration/observation.json
}
brief_json() { "$RB" brief --repo . --this-checkout --format json 2>/dev/null || fail "brief failed"; }
# the check: the integration section of $1 is stale with its remedy, and the line says so
shown_stale() {
  printf '%s' "$1" | jq -e '.integration.observed.verdict == "stale" and .integration.stale == true and (.integration.observed.remedy | startswith("majordomus prs refresh") or contains("&& majordomus prs refresh")) and (.line | test("prs stale [0-9]+[smhd]")) and (.line | test(" open, next #") | not)' >/dev/null
}

git update-ref refs/remotes/origin/master "$now"

# ---------------------------------------------------------------- 1  past its bound
observe 7200 "$now"
b="$(brief_json)"
shown_stale "$b" || { printf '%s' "$b" | jq '{line, integration}'; fail "an observation two hours old is not shown stale"; }
printf '%s' "$b" | jq -e '.line | test("prs stale 2h")' >/dev/null || fail "the line does not say its age: $(printf '%s' "$b" | jq -r .line)"
printf '%s' "$b" | jq -e '.attention | map(.id) | index("integration.stale") != null' >/dev/null \
  || fail "the stale queue is not an attention entry"
printf '%s' "$b" | jq -e '.report.prompt | contains("[stale] integration: queue observed 2 h ago")' >/dev/null \
  || fail "the report prompt does not carry the stale queue verbatim"

# ---------------------------------------------------------------- 2  master moved past it
observe 60 "$base"
b="$(brief_json)"
shown_stale "$b" || { printf '%s' "$b" | jq '{line, integration}'; fail "an observation master moved past is not shown stale"; }
[ "$(printf '%s' "$b" | jq -r '.integration.moved_by')" = 1 ] || fail "it does not say master moved by one commit"

# ---------------------------------------------------------------- 3  current
observe 60 "$now"
b="$(brief_json)"
printf '%s' "$b" | jq -e '.integration.observed.verdict == "current" and .integration.stale == false and (.line | test("prs current"))' >/dev/null \
  || { printf '%s' "$b" | jq '{line, integration}'; fail "a fresh observation at origin/master is not current"; }

# ---------------------------------------------------------------- 4  the check refuses a lie
observe 7200 "$now"
b="$(brief_json)"
patched="$(printf '%s' "$b" | jq '.integration.observed.verdict = "current"')"
if shown_stale "$patched"; then fail "the check accepts a stale observation patched to read current"; fi
exit 0

# majordomus-covers: none
# majordomus-timeout: 900
# A train of stacked pull requests is a typed object, and every surface renders the same one
# (ADR 0138, I2463): a scripted forge as in case 740, one refresh, then only reads.
#
#   1. #1 targets master; #2 targets #1's head branch (a base-branch stack); #3 targets master
#      and says `Stacked on #2.` in its body; #4 stands alone. refs/prep/1 is #1's head
#      (pushed), refs/prep/2 a commit the forge has never seen (pending), refs/prep/9 names a
#      pull request that is not open
#   2. `prs trains --format json` answers one train, train-1, its links in landing order
#      1, 2, 3 with their positions, roots, tops, next link, STACKED_ON edges and prepared
#      heads; #4 is in no train; refs/prep/9 is a prepared head no train holds
#   3. the text rendering says the same, and `majordomus run integration.trains` answers
#      the same value as the command line
#   4. GET /api/v1/pull-requests/trains answers the same trains; /cockpit/trains renders them
#      as a table with the drawing's frame pointing at that route, the navigation links to it
#      and /cockpit/integration links to it; OpenAPI names the route
#   5. none of it reached the forge, and no ref moved
#   6. a checkout that never observed the forge gets `observed: false` and exit 10
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v curl >/dev/null 2>&1 || skip "no curl"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-1072.git"; W="$T/../work-1072"; STATE="$T/../forge-1072"; BIN="$T/../bin-1072"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE" "$BIN"
SRV=""; trap '[ -n "$SRV" ] && kill "$SRV" 2>/dev/null' EXIT
gitq init -q --bare -b master "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
cd "$W" || exit 1
echo base > a.txt; gitq add a.txt; gitq commit -qm base; gitq push -q origin HEAD:master
branch() {   # <number> <from>
  gitq checkout -qb "feature/$1" "$2"; echo "$1" > "f$1.txt"; gitq add "f$1.txt"; gitq commit -qm "$1"
  gitq push -q origin "HEAD:refs/heads/feature/$1" "HEAD:refs/pull/$1/head"
}
branch 1 master
branch 2 feature/1
branch 3 feature/2
branch 4 master
gitq checkout -q master
"$MJ" init >"$STATE/init.log" 2>&1 || { echo "    majordomus init failed:"; tail -5 "$STATE/init.log"; exit 1; }

pr() {   # <number> <base> <body>
  printf '{"number":%s,"title":"change %s","author":{"login":"someone"},"headRefName":"feature/%s","headRefOid":"%s","baseRefName":"%s","isDraft":false,"labels":[],"createdAt":"2026-10-0%sT00:00:00Z","updatedAt":"2026-10-0%sT00:00:00Z","body":"%s","statusCheckRollup":[{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS"}],"reviewDecision":"","autoMergeRequest":null,"isCrossRepository":false}' \
    "$1" "$1" "$1" "$(git rev-parse "feature/$1")" "$2" "$1" "$1" "$3"
}
pr 1 master '' > "$STATE/pr-1.json"
pr 2 feature/1 '' > "$STATE/pr-2.json"
pr 3 master 'Stacked on #2.' > "$STATE/pr-3.json"
pr 4 master '' > "$STATE/pr-4.json"
cat > "$BIN/gh" <<EOF
#!/bin/sh
[ -n "\${DECLARATIONS:-}" ] || echo "\$*" >> "$STATE/log"
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"contexts":["ci"]}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list")
    printf '['; sep=''
    for f in "$STATE"/pr-*.json; do printf '%s' "\$sep"; cat "\$f"; sep=','; done
    printf ']\n' ;;
  "pr view") echo OPEN ;;
  "api graphql")
    nodes="\$(DECLARATIONS=1 "\$0" pr list --state open | grep -o '"number":[0-9]*' | sed 's/.*/{&,"authorAssociation":"OWNER","isCrossRepository":false,"timelineItems":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[]}}/' | paste -sd, -)"
    printf '{"data":{"repository":{"pullRequests":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[%s]}}}}\n' "\$nodes" ;;
  *) echo "UNEXPECTED" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
prs() { "$RB" prs --repo "$W" "$@"; }

# ---------------------------------------------------------------- 1. the forge, observed once
prs refresh >/dev/null || { echo "    refresh failed"; cat "$STATE/log"; exit 1; }
# the executor's prepared heads: #1's is its head, #2's a commit the forge has never seen
gitq update-ref refs/prep/1 "$(git rev-parse feature/1)"
pending="$(git commit-tree -p "$(git rev-parse feature/2)" -m 'prepared, not pushed' "$(git rev-parse 'feature/2^{tree}')")"
gitq update-ref refs/prep/2 "$pending"
gitq update-ref refs/prep/9 "$(git rev-parse master)"
refs_before="$(git for-each-ref --format='%(refname) %(objectname)')"
calls_before="$(wc -l < "$STATE/log" | tr -d ' ')"

# ---------------------------------------------------------------- 2. the typed answer
rc=0; j="$(prs trains --format json)" || rc=$?
[ "$rc" = 0 ] || [ "$rc" = 10 ] || { echo "    prs trains --format json exited $rc: $j"; exit 1; }
printf '%s' "$j" | jq -e '.observed == true and .tallies.trains == 1 and .tallies.links == 3' >/dev/null \
  || { echo "    not one train of three links:"; printf '%s' "$j" | jq '{observed, reason, tallies}'; exit 1; }
t="$(printf '%s' "$j" | jq -c '.trains[0]')"
check() {   # <jq filter> <what>
  printf '%s' "$t" | jq -e "$1" >/dev/null || { echo "    $2:"; printf '%s' "$t" | jq .; exit 1; }
}
check '.id == "train-1"' "the train is not named for its root"
check '[.links[].number] == [1,2,3] and [.links[].position] == [1,2,3]' "the links are not in landing order"
check '[.links[].depth] == [0,1,2]' "the depths are not counted from master"
check '.roots == [1] and .tops == [3] and .next == 1' "roots, tops or the next link are wrong"
check '[.edges[] | [.from, .to, .kind, .satisfied]] == [[2,1,"stacked_on",false],[3,2,"stacked_on",false]]' \
  "a base-branch stack and a Stacked-on line are not both STACKED_ON edges"
check '.links[0].prep.state == "pushed" and .links[0].prep.reference == "refs/prep/1"' "#1's prepared head is not pushed"
check ".links[1].prep.state == \"pending\" and .links[1].prep.sha == \"$pending\"" "#2's prepared head is not pending"
check '.links[2].prep == null' "#3 has a prepared head it does not have"
check '.links[0].at_turn == true and .links[1].at_turn == false and .links[2].at_turn == false' "the turn is wrong"
check '.links[0].url == "https://github.com/o/r/pull/1" and .links[0].checks_url == "https://github.com/o/r/pull/1/checks"' \
  "the forge's pages are not linked"
check '.links[1].on == [1] and .links[0].carries == [2] and (.links[0].checks | map(.name)) == ["ci"]' \
  "what a link stands on, carries or was checked by is wrong"
check '(.links[0].disposition | type) == "string" and (.links[0].required_checks | type) == "string" and (.links[0].relation_kind | type) == "string"' \
  "a link lacks its disposition, required checks or relation"
printf '%s' "$j" | jq -e '[.trains[].links[].number] | index(4) == null' >/dev/null || { echo "    the lone #4 is in a train"; exit 1; }
printf '%s' "$j" | jq -e '.unattached_prep == [{"number":9,"sha":"'"$(git rev-parse master)"'","open":false}]' >/dev/null \
  || { echo "    refs/prep/9 is not a prepared head no train holds:"; printf '%s' "$j" | jq .unattached_prep; exit 1; }

# ---------------------------------------------------------------- 3. the text, and the registry
rc=0; text="$(prs trains)" || rc=$?
[ "$rc" = 0 ] || [ "$rc" = 10 ] || { echo "    prs trains exited $rc"; exit 1; }
for want in "1 train(s), 3 link(s)" "train-1  roots #1  tops #3  next #1" "prep pushed" "pending" "refs/prep/9" "on #1"; do
  case "$text" in *"$want"*) ;; *) echo "    the text lacks '$want':"; printf '%s\n' "$text"; exit 1 ;; esac
done
ran="$("$RB" run integration.trains --repo "$W" --input '{}' --quiet)" || { echo "    majordomus run integration.trains failed"; exit 1; }
[ "$(printf '%s' "$ran" | jq -cS '.trains')" = "$(printf '%s' "$j" | jq -cS '.trains')" ] \
  || { echo "    the capability and the command line disagree:"; printf '%s' "$ran" | head -c 600; exit 1; }

# ---------------------------------------------------------------- 4. HTTP and the Cockpit
serve_up "$STATE/out.txt" "$STATE/err.txt" || exit 1
api="$(curl -s -m 120 "$U/api/v1/pull-requests/trains")"
[ "$(printf '%s' "$api" | jq -cS '.trains')" = "$(printf '%s' "$j" | jq -cS '.trains')" ] \
  || { echo "    the HTTP route and the command line disagree:"; printf '%s' "$api" | head -c 600; exit 1; }
code="$(curl -s -m 120 -o "$STATE/page.html" -w '%{http_code}' "$U/cockpit/trains")"
[ "$code" = 200 ] || { echo "    /cockpit/trains answered $code"; exit 1; }
for want in 'id="train-1"' 'STACKED_ON' 'data-mj-trains-src="/api/v1/pull-requests/trains"' 'trains.js' \
  'https://github.com/o/r/pull/2/checks' 'pending' 'pushed' 'Prepared heads no train holds' 'refs/prep/9' 'href="/cockpit/trains"'; do
  grep -qF "$want" "$STATE/page.html" || { echo "    /cockpit/trains does not show '$want'"; exit 1; }
done
curl -s -m 120 "$U/cockpit/integration" | grep -qF 'href="/cockpit/trains"' \
  || { echo "    /cockpit/integration does not link to the trains"; exit 1; }
curl -s -m 60 "$U/openapi.json" | jq -e '.paths | has("/api/v1/pull-requests/trains")' >/dev/null \
  || { echo "    OpenAPI does not name the trains route"; exit 1; }
serve_down

# ---------------------------------------------------------------- 5. a read moves nothing
[ "$(wc -l < "$STATE/log" | tr -d ' ')" = "$calls_before" ] || { echo "    a read reached the forge:"; tail -5 "$STATE/log"; exit 1; }
[ "$(git for-each-ref --format='%(refname) %(objectname)')" = "$refs_before" ] || { echo "    a read moved a ref"; exit 1; }

# ---------------------------------------------------------------- 6. nothing observed
E="$T/../empty-1072"; rm -rf "$E"; mkdir -p "$E"; ( cd "$E" && gitq init -q && echo x > x && gitq add x && gitq commit -qm x && "$MJ" init >/dev/null 2>&1 )
rc=0; u="$("$RB" prs --repo "$E" trains --format json)" || rc=$?
[ "$rc" = 10 ] || { echo "    an unobserved checkout's trains exited $rc"; exit 1; }
printf '%s' "$u" | jq -e '.observed == false and (.reason | length > 0) and .trains == []' >/dev/null \
  || { echo "    an unobserved checkout does not say why it has no trains: $u"; exit 1; }
"$RB" prs --repo "$E" trains 2>/dev/null | grep -q '^no trains: ' || { echo "    the text does not say why there are no trains"; exit 1; }
exit 0

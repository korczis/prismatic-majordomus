# majordomus-covers: none
# A required check the base binds to an app is only that app's check run: another app's check
# run of the name neither passes it nor overrides the bound app's failure, and what is not
# ready is never merged (ADR 0101; rule project.integration-follows-the-current-master).
# Through the real command line against a scripted forge and a local bare origin, as cases 850
# and 857 do. The protection is the live shape, `ci` bound to app 15368, and the forge lists
# three pull requests that contain master. `gh pr list` names no app on any check run, so who
# wrote each is known only from the writers read (`gh api graphql`), which the scripted forge
# answers only when it is called with exactly the arguments the adapter documents.
#
#   1. the first observation. #1: `ci` passed, written by app 15368: ready, the next merge,
#      its evidence `ci (app 15368): passed`. #2: `ci` FAILED from app 15368, then a newer
#      `ci` PASSED from app 99: needs_repair with required_check_failed, the foreign pass
#      overrides nothing. #3: only app 99's passing `ci`: waiting_for_checks, its required
#      checks missing, because another app's run is not the check.
#   2. the writers were read as the adapter reads them: one page for each observation of the
#      open pull requests, owner and name as strings (`-f`), only the page size typed (`-F`),
#      and never the per-pull-request form, since no rollup was truncated.
#   3. `prs drain --max 3` merges #1 only: the forge's log holds one `pr merge 1`, pinned to
#      its head, none for #2 or #3, no --admin, and master contains #1's head alone.
#   4. nothing was asked of the forge that this harness does not answer.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-925.git"; W="$T/../work-925"; STATE="$T/../forge-925"; BIN="$T/../bin-925"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE" "$BIN"
gitq init -q --bare -b master "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
# the person's identity, in the clone: the executor merges and commits here, and a machine
# with none configured (a CI runner) must not decide the outcome
git -C "$W" config user.email t@example.com; git -C "$W" config user.name t
cd "$W" || exit 1
echo base > a.txt; gitq add a.txt; gitq commit -qm base; gitq push -q origin HEAD:master
M0="$(git rev-parse HEAD)"
for n in 1 2 3; do
  gitq checkout -q -b "feature/$n" "$M0"; echo "$n" > "$n.txt"; gitq add "$n.txt"; gitq commit -qm "$n"
  gitq push -q origin "HEAD:refs/heads/feature/$n" "HEAD:refs/pull/$n/head"
done
H1="$(git rev-parse feature/1)"; H2="$(git rev-parse feature/2)"; H3="$(git rev-parse feature/3)"
gitq checkout -q master
"$MJ" init >"$STATE/init.log" 2>&1 || { echo "    majordomus init failed in the scratch clone:"; tail -5 "$STATE/init.log"; exit 1; }

# the checks, as `gh pr list` projects them: a check run names no app there
run() {   # <conclusion> <started> <completed>
  printf '{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"%s","startedAt":"%s","completedAt":"%s"}' "$1" "$2" "$3"
}
passed="$(run SUCCESS 2026-09-01T00:00:00Z 2026-09-01T00:05:00Z)"
failed="$(run FAILURE 2026-09-01T00:00:00Z 2026-09-01T00:05:00Z)"
later="$(run SUCCESS 2026-09-01T00:08:00Z 2026-09-01T00:09:00Z)"
# and as the GraphQL rollup answers them: the same entry, with the app of its suite
by() {   # <entry> <app>
  printf '%s,"checkSuite":{"app":{"databaseId":%s}}}' "${1%\}}" "$2"
}
listed() {   # <number> <head> <rollup entries>
  printf '{"number":%s,"title":"change %s","author":{"login":"someone"},"headRefName":"feature/%s","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"2026-09-0%sT00:00:00Z","updatedAt":"2026-09-0%sT00:00:00Z","body":"","statusCheckRollup":[%s],"reviewDecision":"","latestReviews":[],"reviewRequests":[],"autoMergeRequest":null,"isCrossRepository":false}' \
    "$1" "$1" "$1" "$2" "$1" "$1" "$3"
}
written() {   # <number> <head> <contexts>
  printf '{"number":%s,"headRefOid":"%s","statusCheckRollup":{"contexts":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[%s]}}}' \
    "$1" "$2" "$3"
}
# #1: the bound app's pass. #2: the bound app's failure, then app 99's newer pass. #3: app 99's
# pass alone. One file per pull request for each read, so a merged one stops being listed.
listed 1 "$H1" "$passed" > "$STATE/pr-1.json"
listed 2 "$H2" "$failed,$later" > "$STATE/pr-2.json"
listed 3 "$H3" "$passed" > "$STATE/pr-3.json"
written 1 "$H1" "$(by "$passed" 15368)" > "$STATE/w-1.json"
written 2 "$H2" "$(by "$failed" 15368),$(by "$later" 99)" > "$STATE/w-2.json"
written 3 "$H3" "$(by "$passed" 99)" > "$STATE/w-3.json"
for f in "$STATE"/pr-?.json "$STATE"/w-?.json; do
  jq -e . "$f" >/dev/null || { echo "    the fixture $f is not JSON"; exit 1; }
done
# the query the adapter sends (forge.rs WRITERS_QUERY), one line: the forge answers no other
cat > "$STATE/query" <<'Q'
query($owner:String!,$name:String!,$n:Int!,$after:String){repository(owner:$owner,name:$name){pullRequests(states:OPEN,first:$n,after:$after,orderBy:{field:CREATED_AT,direction:DESC}){pageInfo{hasNextPage endCursor}nodes{number headRefOid statusCheckRollup{contexts(first:100){pageInfo{hasNextPage endCursor}nodes{__typename ...on CheckRun{name status conclusion startedAt completedAt checkSuite{app{databaseId}}}...on StatusContext{context state startedAt:createdAt}}}}}}}}
Q

# the forge. The open list and the writers page answer the pull requests not yet merged,
# newest first, as GitHub does; the writers page is answered only for the exact argv of the
# page read (ten entries, no cursor), anything else is UNEXPECTED
cat > "$BIN/gh" <<EOF
#!/bin/sh
echo "\$*" >> "$STATE/log"
case " \$* " in *" --admin "*) echo "ADMIN" >> "$STATE/log"; exit 1 ;; esac
open() {   # <prefix> <separator-free JSON files of the open pull requests, newest first>
  sep=''
  for n in 3 2 1; do
    [ -f "$STATE/merged-\$n" ] && continue
    printf '%s' "\$sep"; cat "$STATE/\$1-\$n.json"; sep=','
  done
}
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"checks":[{"context":"ci","app_id":15368}],"contexts":["ci"]}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list")
    case " \$* " in
      *" --state closed "*) echo '[]' ;;
      *" --state open "*) printf '['; open pr; printf ']\n' ;;
      *) echo "UNEXPECTED" >> "$STATE/log"; exit 1 ;;
    esac ;;
  "api graphql")
    asked=no
    if [ "\$#" = 10 ] && [ "\$3|\$5|\$6|\$7|\$8|\$9|\${10}" = "-f|-f|owner=o|-f|name=r|-F|n=50" ]; then
      [ "\$4" = "query=\$(cat "$STATE/query")" ] && asked=yes
    fi
    [ "\$asked" = yes ] || { echo "UNEXPECTED" >> "$STATE/log"; exit 1; }
    printf '{"data":{"repository":{"pullRequests":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":['
    open w
    printf ']}}}}\n' ;;
  "pr merge")
    n="\$3"; want=''; prev=''
    for a in "\$@"; do [ "\$prev" = --match-head-commit ] && want="\$a"; prev="\$a"; done
    have="\$(git -C "$ORIGIN" rev-parse "refs/pull/\$n/head")"
    [ -z "\$want" ] || [ "\$want" = "\$have" ] || { echo "head moved" >&2; exit 1; }
    t="\$(mktemp -d)"
    git clone -q "$ORIGIN" "\$t/c" 2>/dev/null && cd "\$t/c" &&
      git fetch -q origin "refs/pull/\$n/head" &&
      git -c user.email=f@example.com -c user.name=forge merge -q --no-ff FETCH_HEAD -m "Merge pull request #\$n" &&
      git push -q origin HEAD:master && touch "$STATE/merged-\$n" ;;
  "pr view")
    # the executor's read of whether its merge is visible
    case " \$* " in
      *" --json state --jq .state "*) if [ -f "$STATE/merged-\$3" ]; then echo MERGED; else echo OPEN; fi ;;
      *) echo "UNEXPECTED" >> "$STATE/log"; exit 1 ;;
    esac ;;
  *) echo "UNEXPECTED" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
prs() { "$RB" prs --repo "$W" "$@"; }
: > "$STATE/log"
unexpected() {   # <when>: the forge was asked something it does not answer
  if grep -q UNEXPECTED "$STATE/log"; then
    echo "    $1: the forge was asked something this harness does not answer:"
    grep -B1 UNEXPECTED "$STATE/log" | cut -c1-200 | head -4
    exit 1
  fi
}
field() { printf '%s' "$q" | jq -r --argjson n "$1" ".assessments[] | select(.number == \$n) | $2"; }
expect_field() {   # <number> <jq path> <want> <what>
  local got
  got="$(field "$1" "$2")"
  [ "$got" = "$3" ] || { echo "    #$1 ($4): $2 is ${got:-nothing}, not $3"; field "$1" '{disposition, reasons, required_checks}' | sed 's/^/      /'; exit 1; }
}
count() { local c; c="$(grep -c -- "$1" "$STATE/log")" || :; printf '%s' "${c:-0}"; }

# ---------------------------------------------------------------- 1. the first observation
expect_exit 0 prs refresh || { unexpected "the first refresh"; tail -5 "$STATE/log" | cut -c1-200; exit 1; }
unexpected "the first refresh"
# status exits non-zero when it has a diagnostic; the queue it prints is the subject either way
q="$(prs status --format json 2>/dev/null)" || :
[ -n "$q" ] || { echo "    status printed no queue"; exit 1; }

expect_field 1 .disposition ready "ci passed, written by the bound app"
expect_field 1 .required_checks passed "ci passed, written by the bound app"
detail="$(field 1 '.evidence[] | select(.kind == "required_checks") | .detail')"
case "$detail" in *"ci (app 15368): passed"*) ;; *) echo "    #1's evidence does not name the bound check as passed: ${detail:-nothing}"; exit 1 ;; esac
next="$(printf '%s' "$q" | jq -r .next_merge)"
[ "$next" = 1 ] || { echo "    the next merge is $next, not #1, the one pull request whose bound check passed"; exit 1; }

expect_field 2 .disposition needs_repair "the bound app's ci failed, app 99's newer ci passed"
expect_field 2 '.reasons[0]' required_check_failed "the bound app's ci failed, app 99's newer ci passed"
expect_field 2 .required_checks failed "the bound app's ci failed, app 99's newer ci passed"
detail="$(field 2 '.evidence[] | select(.kind == "required_checks") | .detail')"
case "$detail" in *"ci (app 15368): failed"*) ;; *) echo "    #2's evidence does not name the bound check as failed: ${detail:-nothing}"; exit 1 ;; esac

expect_field 3 .disposition waiting_for_checks "only app 99's ci passed"
expect_field 3 '.reasons[0]' required_checks:missing "only app 99's ci passed"
expect_field 3 .required_checks missing "only app 99's ci passed"
detail="$(field 3 '.evidence[] | select(.kind == "required_checks") | .detail')"
case "$detail" in *"ci (app 15368): missing"*) ;; *) echo "    #3's evidence does not name the bound check as missing: ${detail:-nothing}"; exit 1 ;; esac

# ---------------------------------------------------------------- 2. the writers read
grep -q '^api graphql -f query=query(.* -f owner=o -f name=r -F n=50$' "$STATE/log" \
  || { echo "    the writers were not read as the adapter reads them:"; grep '^api' "$STATE/log" | cut -c1-160; exit 1; }
[ "$(count '^api graphql ')" = 1 ] || { echo "    one observation asked for the writers $(count '^api graphql ') times, not once"; exit 1; }

# ---------------------------------------------------------------- 3. the drain merges #1 only
out="$(prs drain --max 3 2>&1)" || { echo "    the drain failed:"; printf '%s\n' "$out" | sed 's/^/      /'; unexpected "the drain"; tail -8 "$STATE/log" | cut -c1-200; exit 1; }
unexpected "the drain"
merged="$(grep '^pr merge' "$STATE/log" | awk '{print $3}' | tr '\n' ' ')" || :
[ "$merged" = "1 " ] || { echo "    the drain merged [$merged], not #1 alone:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
grep -q "^pr merge 1 --merge --match-head-commit $H1\$" "$STATE/log" \
  || { echo "    #1 was not merged pinned to its head:"; grep '^pr merge' "$STATE/log"; exit 1; }
if grep -qE '^pr merge (2|3) ' "$STATE/log"; then echo "    #2 or #3 reached the forge's merge"; exit 1; fi
if grep -q ADMIN "$STATE/log"; then echo "    a call carried --admin"; exit 1; fi
master="$(git -C "$ORIGIN" rev-parse master)"
git -C "$ORIGIN" merge-base --is-ancestor "$H1" "$master" || { echo "    master does not contain #1's head"; exit 1; }
for h in "$H2" "$H3"; do
  if git -C "$ORIGIN" merge-base --is-ancestor "$h" "$master"; then echo "    master contains $h, a head whose bound check never passed"; exit 1; fi
done
# every observation of the open pull requests read the writers, once, by the page form
lists="$(count '^pr list --state open ')"; pages="$(count '^api graphql ')"
# the refresh, the drain's first look, and the second look its merge decision is re-taken from
[ "$lists" -ge 3 ] || { echo "    the drain did not re-take its decision from a new observation: $lists open list(s) in all"; exit 1; }
[ "$pages" = "$lists" ] || { echo "    $lists observation(s) of the open pull requests, $pages writers read(s): not one each"; exit 1; }
if grep -q -- '-F number=' "$STATE/log"; then echo "    a per-pull-request writers read was asked for, and no rollup was truncated"; exit 1; fi

# ---------------------------------------------------------------- 4. the forge's log
unexpected "at the end"
echo "    bound to app 15368: its pass is ready and merged; its failure stands under app 99's newer pass; app 99's pass alone is missing"

# majordomus-covers: none
# A pull request a declared successor replaced is `superseded` once that successor landed, and
# only then; strong evidence from git is `redundant`; weak evidence is never closed (ADR 0101
# §3 and §6, owner decision D2). Through the real command line against a scripted forge, as
# case 720 does:
#
#   1. #2 said "Supersedes #1" and was merged with a merge commit, so it is no longer open: the
#      observation reads it among the closed pull requests that declare a supersession, and #1
#      — which now conflicts with what #2 brought — is superseded, superseded_by 2
#   2. #3 says "Superseded by #4", and #4 was closed unmerged: the observation views #4, and #3
#      is possibly_redundant; #5 says "Superseded by #6", still open: #5 waits, never merges
#   3. cleanup lists #1 to close and leaves #3 to a person; with --apply only #1 is closed, its
#      comment opens with its successor, and the trail records closed_superseded
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-861.git"; W="$T/../work-861"; STATE="$T/../forge-861"; BIN="$T/../bin-861"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE" "$BIN"
gitq init -q --bare -b master "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
cd "$W" || exit 1
echo base > a.txt; gitq add a.txt; gitq commit -qm base; gitq push -q origin HEAD:master
branch() {   # <number> <file> <content>: a branch from master, pushed as its pull request's head
  gitq checkout -qb "feature/$1" master; echo "$3" > "$2"; gitq add "$2"; gitq commit -qm "change $1"
  gitq push -q origin "HEAD:refs/heads/feature/$1" "HEAD:refs/pull/$1/head"; gitq checkout -q master
}
branch 1 a.txt one; branch 2 a.txt two; branch 3 three.txt three; branch 4 four.txt four
branch 5 five.txt five; branch 6 six.txt six
H() { git --no-pager rev-parse "feature/$1"; }
# #2 lands with a merge commit, as the forge merges: #1 conflicts with master now
gitq merge -q --no-ff "feature/2" -m "Merge pull request #2"; gitq push -q origin HEAD:master
"$MJ" init >"$STATE/init.log" 2>&1 || { echo "    majordomus init failed:"; tail -5 "$STATE/init.log"; exit 1; }

ci='{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS","completedAt":"2026-09-01T00:05:00Z"}'
pr() {   # <number> <body>
  printf '{"number":%s,"title":"change %s","author":{"login":"someone"},"headRefName":"feature/%s","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"2026-09-0%sT00:00:00Z","updatedAt":"2026-09-0%sT00:00:00Z","body":"%s","statusCheckRollup":[%s],"reviewDecision":"","latestReviews":[],"reviewRequests":[],"autoMergeRequest":null,"isCrossRepository":false}' \
    "$1" "$1" "$1" "$(H "$1")" "$1" "$1" "$2" "$ci"
}
closed() {   # <number> <state> <body>
  printf '{"number":%s,"state":"%s","headRefOid":"%s","body":"%s"}' "$1" "$2" "$(H "$1")" "$3"
}
printf '[%s,%s,%s,%s]\n' "$(pr 1 '')" "$(pr 3 'Superseded by #4')" "$(pr 5 'Superseded by #6')" "$(pr 6 '')" > "$STATE/open.json"
printf '[%s]\n' "$(closed 2 MERGED 'Supersedes #1')" > "$STATE/closed.json"
closed 4 CLOSED '' > "$STATE/view-4.json"

cat > "$BIN/gh" <<EOF
#!/bin/sh
echo "\$*" >> "$STATE/log"
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"contexts":["ci"]}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list")
    case " \$* " in *" --state closed "*) cat "$STATE/closed.json" ;; *) cat "$STATE/open.json" ;; esac ;;
  "pr view")
    if [ -f "$STATE/view-\$3.json" ]; then cat "$STATE/view-\$3.json"; else echo "UNEXPECTED" >> "$STATE/log"; exit 1; fi ;;
  "pr close") echo "CLOSE \$3" >> "$STATE/log" ;;
  *) echo "UNEXPECTED" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
prs() { "$RB" prs --repo "$W" "$@"; }
field() { printf '%s' "$q" | jq -r --argjson n "$1" ".assessments[] | select(.number == \$n) | $2"; }

# ---------------------------------------------------------------- 1. a successor that landed
prs refresh >/dev/null || { echo "    refresh failed"; tail -5 "$STATE/log"; exit 1; }
q="$(prs status --format json 2>/dev/null)" || :
[ -n "$q" ] || { echo "    status printed no queue"; exit 1; }
[ "$(field 1 .disposition)" = superseded ] || { echo "    #1 is $(field 1 .disposition), not superseded"; field 1 '{reasons, evidence}'; exit 1; }
[ "$(field 1 .superseded_by)" = 2 ] || { echo "    #1 is not superseded by #2: $(field 1 .superseded_by)"; exit 1; }
[ "$(field 1 '.reasons[0]')" = 'superseded_by:#2' ] || { echo "    #1's first reason is $(field 1 '.reasons[0]')"; exit 1; }
[ "$(field 1 '.evidence[] | select(.kind == "supersession") | .status')" = landed ] \
  || { echo "    #1's supersession evidence does not say landed"; field 1 .evidence; exit 1; }
grep -q -- '^pr list --state closed --search supersedes in:body' "$STATE/log" \
  || { echo "    the closed pull requests declaring a supersession were not read:"; cat "$STATE/log"; exit 1; }
ex="$(prs explain 1)" || { echo "    explain 1 failed"; exit 1; }
case "$ex" in *"superseded:   by #2"*) ;; *) echo "    explain does not name #1's successor: $ex"; exit 1 ;; esac

# ---------------------------------------------------------------- 2. a successor that did not land, and one open
[ "$(field 3 .disposition)" = possibly_redundant ] || { echo "    #3 is $(field 3 .disposition), not possibly_redundant"; exit 1; }
[ "$(field 3 '.reasons[0]')" = 'successor_not_landed:#4' ] || { echo "    #3's first reason is $(field 3 '.reasons[0]')"; exit 1; }
[ "$(field 3 .superseded_by)" = null ] || { echo "    #3 names a successor that did not land"; exit 1; }
[ "$(field 5 .disposition)" = waiting_for_dependency ] || { echo "    #5 is $(field 5 .disposition), not waiting_for_dependency"; exit 1; }
[ "$(field 5 '.reasons[0]')" = 'successor_open:#6' ] || { echo "    #5's first reason is $(field 5 '.reasons[0]')"; exit 1; }
# only the named successor that is not open was viewed: not #6 (open), not #2 (read closed)
[ "$(grep -c '^pr view' "$STATE/log")" = 1 ] && grep -q '^pr view 4 ' "$STATE/log" \
  || { echo "    the successors viewed are not exactly #4:"; grep '^pr view' "$STATE/log"; exit 1; }

# ---------------------------------------------------------------- 3. cleanup
out="$(prs cleanup --format json)" || { echo "    cleanup failed: $out"; exit 1; }
[ "$(printf '%s' "$out" | jq -c '[.[] | [.pr, .disposition, .action]]')" = '[[1,"superseded","would_close"],[3,"possibly_redundant","left_for_a_person"]]' ] \
  || { echo "    the dry cleanup does not list #1 to close and #3 for a person: $out"; exit 1; }
grep -q '^CLOSE' "$STATE/log" && { echo "    a cleanup without --apply closed something"; exit 1; }
prs cleanup --apply >/dev/null || { echo "    cleanup --apply failed"; exit 1; }
[ "$(grep -c '^CLOSE' "$STATE/log")" = 1 ] && grep -q '^CLOSE 1$' "$STATE/log" \
  || { echo "    not exactly #1 was closed:"; grep '^CLOSE' "$STATE/log"; exit 1; }
grep -q '^pr close 1 --comment Superseded by #2, which landed\.' "$STATE/log" \
  || { echo "    #1's closing comment does not open with its successor:"; grep -A1 '^pr close' "$STATE/log"; exit 1; }
ev="$W/.git/majordomus/integration/events.jsonl"
jq -e 'select(.action == "closed_superseded" and .pr == 1)' "$ev" >/dev/null \
  || { echo "    the trail does not record #1 as closed_superseded"; cat "$ev"; exit 1; }
grep -q UNEXPECTED "$STATE/log" && { echo "    the forge was asked something this harness does not answer:"; grep -B1 UNEXPECTED "$STATE/log" | head -4; exit 1; }
echo "    a successor that landed supersedes; one open holds; one that did not land is a person's"

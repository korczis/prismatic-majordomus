# majordomus-covers: none
# The record pull request of a release is opened with the workflow's own token, and a forge
# starts no workflow from an event that token caused: the pull request has no check of its
# own, so it can never merge until somebody closes and reopens it (I2282, I2296). On v0.19.1
# that took a person five hours to notice, with every open head red meanwhile.
#
# `majordomus prs drain` does it itself, through the real command line, against a scripted
# forge and a local bare origin:
#
#   - a `release/record-<tag>` pull request the workflow's token opened, older than the
#     grace and with an empty rollup, is closed and reopened: under the integration lease,
#     `reopen_attempted` on the trail before the forge is asked, `reopened` after;
#   - a dry run only names it;
#   - the same head is never reopened twice, though it reads `missing` until its run reports;
#   - a record a person opened is left alone, and so is every other branch;
#   - once its own check passes the record merges like any other pull request.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-1036.git"; W="$T/../work-1036"; STATE="$T/../forge-1036"; BIN="$T/../bin-1036"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE" "$BIN"
gitq init -q --bare -b master "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
cd "$W" || exit 1
git config user.email t@example.com; git config user.name t
common="$(git rev-parse --path-format=absolute --git-common-dir)"
TRAIL="$common/majordomus/integration/events.jsonl"
echo base > a.txt; gitq add -A; gitq commit -qm base; gitq push -q origin HEAD:master
# pull request <n> from <branch>: one commit on master, pushed as the branch and as its head
head_of() {
  gitq checkout -q -B "$2" origin/master; echo "$1" > "$1.txt"
  gitq add -A; gitq commit -qm "$2"
  gitq push -q origin "HEAD:refs/heads/$2" "HEAD:refs/pull/$1/head"
  git rev-parse HEAD; gitq checkout -q --detach origin/master
}
H1="$(head_of 1 release/record-v9.9.9)"
H2="$(head_of 2 release/record-v9.9.8)"
H3="$(head_of 3 feature/other)"
PASSED='[{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS","completedAt":"2026-10-10T03:30:00Z"}]'
# <n> <branch> <head> <author> <rollup>: one pull request as the forge lists it
pr() {
  printf '{"number":%s,"title":"t%s","author":{"login":"%s"},"headRefName":"%s","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"2026-10-10T01:56:10Z","updatedAt":"2026-10-10T01:56:10Z","body":"","statusCheckRollup":%s,"reviewDecision":"","latestReviews":[],"reviewRequests":[],"autoMergeRequest":null,"isCrossRepository":false}' "$1" "$1" "$4" "$2" "$3" "$5"
}
# #1 the workflow's token opened; #2 a person opened; #3 is not a record at all
listing() {
  printf '[%s,%s,%s]\n' \
    "$(pr 1 release/record-v9.9.9 "$H1" app/github-actions "$1")" \
    "$(pr 2 release/record-v9.9.8 "$H2" korczis '[]')" \
    "$(pr 3 feature/other "$H3" korczis '[]')" > "$STATE/prs.json"
}
listing '[]'

cat > "$BIN/gh" <<EOF
#!/bin/sh
[ -n "\${QUIET:-}" ] || echo "\$*" >> "$STATE/log"
# the pull requests still open: a closed or merged one leaves the listing
open() { GONE="\$(ls "$STATE" | sed -n 's/^gone-//p' | tr '\n' ' ')" jq -c '[.[] | select((.number|tostring) as \$n | (\$ENV.GONE | split(" ") | index(\$n)) | not)]' "$STATE/prs.json"; }
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"strict":true,"contexts":["ci"]}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list") open ;;
  "api graphql")
    open | jq -c '{data: {repository: {pullRequests: {
      pageInfo: {hasNextPage: false, endCursor: null},
      nodes: [.[] | {number, authorAssociation: "OWNER", isCrossRepository: false, timelineItems: {
        pageInfo: {hasNextPage: false, endCursor: null}, nodes: []}}]}}}}' ;;
  "pr view")
    n="\$3"; state=OPEN; [ -f "$STATE/gone-\$n" ] && state="\$(cat "$STATE/gone-\$n")"
    case " \$* " in
      *" --json state,headRefOid,author,createdAt,statusCheckRollup "*)
        jq -c --argjson n "\$n" --arg state "\$state" '.[] | select(.number == \$n) | {state: \$state, headRefOid, author, createdAt, statusCheckRollup}' "$STATE/prs.json" ;;
      *" --json state,headRefOid "*)
        jq -r --argjson n "\$n" --arg state "\$state" '.[] | select(.number == \$n) | \$state + " " + .headRefOid' "$STATE/prs.json" ;;
      *) echo "\$state" ;;
    esac ;;
  "pr close") echo CLOSED > "$STATE/gone-\$3" ;;
  "pr reopen") rm -f "$STATE/gone-\$3"; echo "\$3" >> "$STATE/reopened" ;;
  "pr merge")
    n="\$3"; t="\$(mktemp -d)"
    git clone -q "$ORIGIN" "\$t/c" 2>/dev/null && cd "\$t/c" && git fetch -q origin "refs/pull/\$n/head" &&
      git -c user.email=f@example.com -c user.name=forge merge -q --no-ff FETCH_HEAD -m "Merge pull request #\$n" &&
      git push -q origin HEAD:master && echo MERGED > "$STATE/gone-\$n" ;;
  *) echo "UNEXPECTED \$*" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
prs() { "$RB" prs --repo "$W" "$@"; }
actions() { jq -r .action "$TRAIL" | tr '\n' ' '; }
said() { grep -c "^$1" "$STATE/log" || true; }

# ---------------------------------------------------------------- a dry run only names it
expect_exit 0 prs drain --dry-run
expect_grep 'would close and reopen #1'
[ "$(said 'pr close')$(said 'pr reopen')" = 00 ] || { echo "    a dry run reached the forge:"; cat "$STATE/log"; exit 1; }

# ---------------------------------------------------------------- the stranded record is reopened
: > "$TRAIL"
expect_exit 0 prs drain --max 1
expect_grep 'closed and reopened #1'
[ "$(cat "$STATE/reopened")" = 1 ] || { echo "    #1 was not reopened, or something else was:"; cat "$STATE/log"; exit 1; }
[ ! -e "$STATE/gone-1" ] || { echo "    #1 was left closed"; exit 1; }
# the closure carries the reason, and comes before the reopen
c="$(grep -n '^pr close 1 --comment' "$STATE/log" | head -n 1 | cut -d: -f1)"
r="$(grep -n '^pr reopen 1' "$STATE/log" | head -n 1 | cut -d: -f1)"
[ -n "$c" ] && [ -n "$r" ] && [ "$c" -lt "$r" ] || { echo "    close and reopen are not in that order:"; cat "$STATE/log"; exit 1; }
grep -q "^pr close 1 --comment .*workflow's own token" "$STATE/log" || { echo "    the closure does not say why"; exit 1; }
# under the lease, and on the trail before the forge was asked
case "$(actions)" in
  "lease_acquired "*" reopen_attempted reopened lease_released ") ;;
  *) echo "    the trail does not hold the reopen under the lease: $(actions)"; exit 1 ;;
esac
[ "$(jq -r 'select(.action == "reopen_attempted") | "\(.pr) \(.head_sha)"' "$TRAIL")" = "1 $H1" ] \
  || { echo "    reopen_attempted does not name #1 at its head"; exit 1; }
# the person's record and the other branch were asked nothing and touched by nothing
if grep -q '^pr \(close\|reopen\) [23]\|UNEXPECTED\|pr merge' "$STATE/log"; then echo "    the forge was asked to act on another pull request:"; cat "$STATE/log"; exit 1; fi
[ "$(said 'pr view 3 --json state,headRefOid,author')" = 0 ] || { echo "    a branch that is no record was asked after"; exit 1; }

# ---------------------------------------------------------------- once per head
expect_exit 0 prs drain --max 1
expect_exit 0 prs drain --max 1
[ "$(wc -l < "$STATE/reopened" | tr -d ' ')" = 1 ] || { echo "    the same head was reopened again:"; cat "$STATE/log"; exit 1; }

# ---------------------------------------------------------------- its own check passes: it merges
listing "$PASSED"
expect_exit 0 prs drain --max 1
expect_grep 'merged #1'
[ "$(git -C "$ORIGIN" rev-parse 'master^2')" = "$H1" ] || { echo "    master does not hold the record's head"; exit 1; }
echo "    a record no run was started for is reopened once, under the lease and trail first, and merges when its check passes"

# majordomus-covers: none
# A pull request shows its trail (I2473), through a real server over a fixture with a recorded
# forge observation and an executor trail:
#
#   the inspector   ?pr=5 shows what the executor did to #5, newest first, and nothing it did
#                   to another pull request
#   the activity    the drain lists the lane's recent events about pull requests, each leading
#                   to that pull request's inspector; an event about none is not listed
#
# The unit tests in apps/majordomus-cli/src/ui/prs.rs own the rendering; this case owns the
# seam: the trail file the executor writes under the common git directory, read by the
# executable a person runs, on the route the screen asks.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# the fixture's server listens on loopback whatever the machine's own default host is
unset MAJORDOMUS_HTTP_HOST

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install
git branch -M master
base="$(git rev-parse HEAD)"
git checkout -q -b feature/five
printf 'five\n' > five.txt && git add five.txt && git commit -qm "the fifth change"
head="$(git rev-parse HEAD)"
git checkout -q master
git update-ref refs/majordomus/prs/5 "$head"
git update-ref refs/remotes/origin/master "$base"

mkdir -p .ai/local/state/integration
cat > .ai/local/state/integration/observation.json <<J
{"schema":4,"repository":"o/r","base":"master","base_sha":"$base","observed_at":"2026-10-11T12:00:00Z",
 "required_checks":null,"review_policy":null,"merge_methods":["merge"],"pull_requests":[
 {"number":5,"title":"the fifth change","author":"a","head_ref":"feature/five","head_sha":"$head","base_ref":"master",
  "draft":false,"created_at":"2026-10-11T00:00:00Z","updated_at":"2026-10-11T00:00:00Z","checks":[]}]}
J
trail="$(git rev-parse --git-common-dir)/majordomus/integration"
mkdir -p "$trail"
ev() { printf '{"at":"%s","actor":"executor","action":"%s","pr":%s,"master_before":null,"head_sha":null,"master_after":null,"reasons":[%s],"detail":"%s"}\n' "$@"; }
{
  ev 2026-10-11T01:00:00Z observed null '' "1 open"
  ev 2026-10-11T02:00:00Z refresh_selected 5 '"behind_master:1"' "master is ahead"
  ev 2026-10-11T03:00:00Z merge_attempted 6 '' "another one"
  ev 2026-10-11T04:00:00Z repaired 5 '' "derived again"
} > "$trail/events.jsonl"

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1
get() { curl -s --max-time 120 "$U$1"; }

drain="$(get '/ui/panels/drain?fragment&pr=5')"
printf '%s' "$drain" | grep -q 'What the executor did' \
  || { echo "    ?pr=5 opened no trail:"; printf '%s\n' "$drain" | grep -o 'mj-ui-empty">[^<]*' | sed 's/^/    | /'; echo; exit 1; }

# ---------------------------------------------------------------- the inspector
own="$(printf '%s' "$drain" | sed -n 's/.*What the executor did\(.*\)/\1/p' | sed 's#</ol>.*##')"
printf '%s' "$own" | grep -q '#5 repaired' && printf '%s' "$own" | grep -q '#5 refresh selected' \
  || { echo "    the inspector does not show what the executor did to #5"; exit 1; }
printf '%s' "$own" | grep -q '#6' && { echo "    the inspector shows another pull request's event"; exit 1; }
first="$(printf '%s' "$own" | grep -o '#5 [a-z ]*' | head -n 1)"
[ "$first" = "#5 repaired" ] || { echo "    the trail is not newest first: '$first'"; exit 1; }

# ---------------------------------------------------------------- the activity
activity="$(printf '%s' "$drain" | sed -n 's/.*id="activity"\(.*\)/\1/p')"
printf '%s' "$activity" | grep -q '#6 merge attempted' \
  || { echo "    the drain does not list the lane's recent activity"; exit 1; }
printf '%s' "$activity" | grep -qF 'href="/ui/prs?pr=6#pr"' \
  || { echo "    an event does not lead to its pull request"; exit 1; }
printf '%s' "$activity" | grep -q '>observed<' && { echo "    an event about no pull request is listed as activity"; exit 1; }

echo "    a pull request shows what the executor did to it, and the drain what it did lately"

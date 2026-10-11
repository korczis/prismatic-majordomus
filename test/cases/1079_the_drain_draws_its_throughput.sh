# majordomus-covers: none
# The PR Drain draws its merge throughput (I2474), through a real server over a fixture with a
# recorded forge observation and an executor trail:
#
#   the window      fourteen days ending on the day of the trail's newest event, not the
#                   reader's clock
#   the counts      each day's successful merges; an attempt is no merge
#   readable        the counts are written in the chart's label and in a table beside it
#
# The unit test in apps/majordomus-cli/src/ui/prs.rs owns the arithmetic; this case owns the
# trail file the executor writes, read by the executable a person runs.
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
  ev 2026-10-09T10:00:00Z merge_succeeded 3 '' "merged and verified"
  ev 2026-10-09T12:00:00Z merge_succeeded 4 '' "merged and verified"
  ev 2026-10-10T09:00:00Z merge_attempted 6 '' "an attempt is no merge"
  ev 2026-10-11T08:00:00Z merge_succeeded 2 '' "merged and verified"
  ev 2026-10-11T09:00:00Z observed null '' "1 open"
} > "$trail/events.jsonl"

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1
drain="$(curl -s --max-time 120 "$U/ui/panels/drain?fragment")"
section="$(printf '%s' "$drain" | sed -n 's/.*id="throughput"\(.*\)/\1/p')"
[ -n "$section" ] || { echo "    the drain draws no throughput"; printf '%s\n' "$drain" | grep -o 'mj-ui-empty">[^<]*' | sed 's/^/    | /'; echo; exit 1; }
label="$(printf '%s' "$section" | grep -o 'Merges per day from [^"]*' | head -n 1)"
[ "$label" = "Merges per day from 2026-09-28 to 2026-10-11: 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 1" ] \
  || { echo "    the chart does not say each day's merges: '$label'"; exit 1; }
printf '%s' "$section" | grep -q '<svg' || { echo "    the throughput has no chart"; exit 1; }
printf '%s' "$section" | grep -qE '2026-10-09</td><td[^>]*>2<' \
  || { echo "    the table does not carry the day's count"; exit 1; }

echo "    the drain counts each day's merges over the fortnight its trail ends on"

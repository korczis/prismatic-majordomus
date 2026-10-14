# majordomus-covers: none
# How long CI takes (I2472), through a real server over a fixture that holds a recorded forge
# observation: integration.timings measures each check from when its run started to when it
# completed, and the CI/CD screen draws it.
#
#   no observation    the route says nothing is observed, and the screen measures nothing
#   an observation    each check's runs, failures, median and 90th percentile, the head that
#                     waited longest, and the run still going counted, never measured as zero
#   the screen        the CI/CD screen draws the checks and leads from a head to its PR
#
# The unit tests in apps/majordomus-cli/src/integration/timings.rs own the arithmetic; this
# case owns the seam: an observation file as `prs refresh` records it, read by the executable
# a person runs, over the route and the screen a person opens.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    jq is not installed"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# the fixture's server listens on loopback whatever the machine's own default host is
unset MAJORDOMUS_HTTP_HOST

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1
get() { curl -s --max-time 60 "$U$1"; }

# ---------------------------------------------------------------- no observation
t="$(get /api/v1/pull-requests/timings)"
[ "$(printf '%s' "$t" | jq -r '.observed')" = false ] \
  || { echo "    a checkout with no observation was measured:"; printf '%s\n' "$t" | sed 's/^/    | /'; echo; exit 1; }
get /ui/delivery | grep -q 'so no run is measured' \
  || { echo "    the CI/CD screen does not say nothing is measured"; exit 1; }

# ---------------------------------------------------------------- an observation
# two heads: #11 ran rust (10 min, passed) and site (40 min, failed); #12 ran rust (30 min,
# passed), has a suite still running, and an old run recorded before starts were read: the
# last two are counted as unmeasured, never timed from nothing
check() { printf '{"name":"%s","state":"%s","kind":"check_run","started_at":"%s","completed_at":"%s"}' "$@"; }
pr() {
  printf '{"number":%s,"title":"pr %s","author":"a","head_ref":"h%s","head_sha":"%s","base_ref":"master","draft":false,"created_at":"2026-10-11T00:00:00Z","updated_at":"2026-10-11T00:00:00Z","checks":[%s]}' \
    "$1" "$1" "$1" "$(printf '%040d' "$1")" "$2"
}
mkdir -p .ai/local/state/integration
cat > .ai/local/state/integration/observation.json <<J
{"schema":4,"repository":"o/r","base":"master","base_sha":"$(printf '%040d' 0)","observed_at":"2026-10-11T12:00:00Z",
 "required_checks":null,"review_policy":null,"merge_methods":["merge"],"pull_requests":[
 $(pr 11 "$(check rust passed 2026-10-11T10:00:00Z 2026-10-11T10:10:00Z),$(check site failed 2026-10-11T10:00:00Z 2026-10-11T10:40:00Z)"),
 $(pr 12 "$(check rust passed 2026-10-11T11:00:00Z 2026-10-11T11:30:00Z),$(check suite pending 2026-10-11T11:00:00Z ''),$(check old passed '' 2026-10-11T11:20:00Z)")]}
J
t="$(get /api/v1/pull-requests/timings)"
[ "$(printf '%s' "$t" | jq -r '.observed')" = true ] \
  || { echo "    a recorded observation was not read:"; printf '%s\n' "$t" | head -c 800 | sed 's/^/    | /'; echo; exit 1; }
got="$(printf '%s' "$t" | jq -r '.timings | [.measured_runs, .unmeasured_runs, .checks[0].name, .checks[0].median_seconds, (.checks[] | select(.name=="rust") | .runs, .median_seconds, .p90_seconds), .heads[0].number, .heads[0].wall_seconds, .heads[0].slowest_check] | map(tostring) | join(" ")')"
[ "$got" = "3 2 site 2400 2 600 1800 11 2400 site" ] \
  || { echo "    the timings are not what the runs took: '$got'"; printf '%s\n' "$t" | head -c 1200 | sed 's/^/    | /'; echo; exit 1; }

# ---------------------------------------------------------------- the MCP tool, as a client calls it
{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case1077","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_ci_timings","arguments":{}}}'
} > mcp.in
"$RB" mcp < mcp.in > mcp.out 2> mcp.err || { echo "    the MCP server failed"; tail -5 mcp.err; exit 1; }
mcp="$(jq -c 'select(.id == 2)' mcp.out)"
printf '%s' "$mcp" | jq -e '.result.isError == false' >/dev/null || { echo "    majordomus_ci_timings failed:"; printf '%s' "$mcp" | head -c 600; exit 1; }
[ "$(printf '%s' "$mcp" | jq -cS '.result.structuredContent.timings')" = "$(printf '%s' "$t" | jq -cS '.timings')" ] \
  || { echo "    the MCP tool and the HTTP route disagree:"; printf '%s' "$mcp" | head -c 600; exit 1; }

# ---------------------------------------------------------------- the screen
# No screen waits for a source (ADR 0142): the first answer may be drawn from a reading that is
# still catching up, and says so. The screen is asked again until it has the reading, for a
# bounded time, and a screen that never draws it fails as it did.
drawn=0
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
  page="$(get /ui/delivery)"
  if printf '%s' "$page" | grep -q '40 min 00 s'; then drawn=1; break; fi
  sleep 0.5
done
[ "$drawn" = 1 ] \
  || { echo "    the CI/CD screen does not draw the slowest check's median, ten seconds after it was first asked"; exit 1; }
printf '%s' "$page" | grep -qF 'href="/ui/prs?pr=11#pr"' \
  || { echo "    the longest wait does not lead to its pull request"; exit 1; }

echo "    CI is measured from each run's start to its end, and a run still going is counted, not timed"

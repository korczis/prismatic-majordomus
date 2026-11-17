# majordomus-covers: none
# The Issues screen of majordomus ui (I2475), through a real server over a real fixture plan:
#
#   the tabs      the plan's own statuses with their counts; one narrows the issues to it
#   the filters   a milestone and a search narrow together, and the pick stays picked
#   the links     an issue opens its inspector, its milestone the milestone's, and what
#                 blocks it that issue's
#   a bad value   a status that is no word of the plan is dropped, never echoed
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# the fixture's server listens on loopback whatever the machine's own default host is
unset MAJORDOMUS_HTTP_HOST

"$MJ" init >/dev/null
pj_init
pj_milestone M1 1
pj_milestone M2 2
pj_issue I0001 M1
pj_issue I0002 M1 I0001
pj_issue I0003 M2
git add -A >/dev/null && git commit -qm "a plan of three issues"

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1
get() { curl -s --max-time 60 "$U$1"; }

# ---------------------------------------------------------------- the tabs
page="$(get /ui/issues)"
printf '%s' "$page" | grep -q '3 of 3 issues shown' \
  || { echo "    /ui/issues does not show every issue:"; printf '%s\n' "$page" | grep -o 'mj-ui-empty">[^<]*' | sed 's/^/    | /'; echo; exit 1; }
blocked="$(printf '%s' "$page" | grep -o 'href="/ui/issues?status=BLOCKED"' | head -n 1)"
[ -n "$blocked" ] || { echo "    no tab for the plan's BLOCKED status"; exit 1; }
get '/ui/issues?status=BLOCKED' | grep -q '1 of 3 issues shown' \
  || { echo "    the BLOCKED tab does not narrow to the one blocked issue"; exit 1; }

# ---------------------------------------------------------------- the filters
page="$(get '/ui/issues?milestone=M1&q=I0002')"
printf '%s' "$page" | grep -q '1 of 3 issues shown' \
  || { echo "    a milestone and a search do not narrow together"; exit 1; }
printf '%s' "$page" | grep -qF 'selected="selected" value="M1"' \
  || { echo "    the picked milestone does not stay picked"; exit 1; }

# ---------------------------------------------------------------- the links
page="$(get '/ui/issues?status=BLOCKED')"
for want in 'href="/ui/prs?issue=I0002#issue"' 'href="/ui/prs?issue=I0001#issue"' 'href="/ui/roadmap?milestone=M1#milestone"'; do
  printf '%s' "$page" | grep -qF "$want" || { echo "    the blocked issue has no link $want"; exit 1; }
done

# ---------------------------------------------------------------- a bad value
page="$(get '/ui/issues?status=READY%22%20onclick%3D%22x')"
printf '%s' "$page" | grep -q '3 of 3 issues shown' \
  || { echo "    a status that is no word narrowed the issues"; exit 1; }
printf '%s' "$page" | grep -q 'onclick' && { echo "    the page echoes a handler a link carried"; exit 1; }

echo "    the plan's issues are browsed by status, milestone and text, each a link onward"

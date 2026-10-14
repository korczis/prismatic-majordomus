# majordomus-covers: none
# The Roadmap's milestone inspector (I2471), through a real server over a real fixture plan:
#
#   a milestone       ?milestone= opens it: its issues lead to their inspectors, its neighbours
#                     to theirs, and its critical path names the open chain in order
#   unknown or bad    an id the plan does not hold says so; a value that is no id opens nothing
#   the way in        the roadmap's rows lead to the inspector
#
# The unit tests in apps/majordomus-cli/src/ui/milestone.rs own the inspector over a plan
# answer; this case owns the route a browser asks, answered by the executable a person runs.
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
printf 'depends_on:\n  - M1\n' >> .ai/repo/project/milestones/M2.yaml
pj_issue I0001 M2
pj_issue I0002 M2 I0001
pj_issue I0003 M2 I0002
pj_issue I0004 M1
git add -A >/dev/null && git commit -qm "a plan of two milestones"

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1
get() { curl -s --max-time 60 "$U$1"; }

# ---------------------------------------------------------------- a milestone
page="$(get '/ui/roadmap?milestone=M2')"
printf '%s' "$page" | grep -q 'id="milestone"' \
  || { echo "    ?milestone=M2 opened no inspector:"; printf '%s' "$page" | grep -o 'mj-ui-empty">[^<]*' | sed 's/^/    | /'; exit 1; }
for want in 'href="/ui/prs?issue=I0001#issue"' 'href="/ui/prs?issue=I0003#issue"' 'href="/ui/roadmap?milestone=M1#milestone"'; do
  printf '%s' "$page" | grep -qF "$want" || { echo "    the inspector has no link $want"; exit 1; }
done
path="$(printf '%s' "$page" | sed -n 's/.*aria-label="The longest chain of open issues">\(.*\)<\/ol>.*/\1/p' | grep -o '>I000[0-9]<' | tr -d '<>' | tr '\n' ' ')"
[ "$path" = "I0001 I0002 I0003 " ] \
  || { echo "    the critical path is not the open chain in order: '$path'"; exit 1; }

# ---------------------------------------------------------------- unknown or bad
get '/ui/roadmap?milestone=M9' | grep -q 'M9 is not a milestone of the plan' \
  || { echo "    an id the plan does not hold is not said to be unknown"; exit 1; }
page="$(get '/ui/roadmap?milestone=M1%22%20onclick%3D%22x')"
printf '%s' "$page" | grep -q 'id="milestone"' && { echo "    a value that is no id opened an inspector"; exit 1; }
printf '%s' "$page" | grep -q 'onclick' && { echo "    the page echoes a handler a link carried"; exit 1; }

# ---------------------------------------------------------------- the way in
get '/ui/roadmap' | grep -qF 'href="/ui/roadmap?milestone=M2#milestone"' \
  || { echo "    the roadmap's rows do not lead to the inspector"; exit 1; }

echo "    a milestone opens, links its issues and neighbours, and names its open chain in order"

# majordomus-covers: none
# The Roadmap's intent inspector (I2476), through a real server over a fixture with an intent,
# a milestone that serves it and an issue that serves one of its criteria:
#
#   an intent       ?intent= opens it: its statement, its criteria with their state, and the
#                   milestone and the issue that serve it, each a link
#   serves          an issue serving a criterion (`intent#criterion`) serves the intent
#   the way in      the roadmap's intent rows, and the milestone inspector's Serves, lead to it
#   unknown or bad  an id the repository does not hold says so; a value that is no id opens
#                   nothing
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
pj_milestone probe 1
pj_issue I0001 probe
printf 'serves:\n  - probe#probe-passes\n' >> .ai/repo/project/issues/I0001.yaml
mkdir -p .ai/repo/project/intents test/cases
printf '. "$ROOT/test/lib.sh"\ntrue\n' > test/cases/01_probe.sh
cat > .ai/repo/project/intents/probe.yaml <<'YAML'
id: probe
title: The probe's outcome is true
statement: "The outcome is true for the people it is for."
invariants:
  - The probe stays a valid repository
milestones:
  - probe
satisfaction:
  - id: probe-passes
    criterion: The probe's case passes
    evidence: test
    ref: test/cases/01_probe.sh
YAML
git add -A >/dev/null && git commit -qm "an intent, its milestone and an issue serving it"

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1
get() { curl -s --max-time 60 "$U$1"; }

# ---------------------------------------------------------------- an intent
page="$(get '/ui/roadmap?intent=probe')"
printf '%s' "$page" | grep -q 'id="intent"' \
  || { echo "    ?intent=probe opened no inspector:"; printf '%s\n' "$page" | grep -o 'mj-ui-empty">[^<]*' | sed 's/^/    | /'; echo; exit 1; }
for want in 'The outcome is true for the people it is for.' 'probe-passes' '0 of 1 criteria met' \
            'href="/ui/roadmap?milestone=probe#milestone"' 'href="/ui/prs?issue=I0001#issue"'; do
  printf '%s' "$page" | grep -qF "$want" || { echo "    the inspector has no $want"; exit 1; }
done

# ---------------------------------------------------------------- the way in
get '/ui/roadmap' | grep -qF 'href="/ui/roadmap?intent=probe#intent"' \
  || { echo "    the roadmap's intent rows do not lead to the inspector"; exit 1; }
get '/ui/roadmap?milestone=probe' | grep -qF 'href="/ui/roadmap?intent=probe#intent"' \
  || { echo "    the milestone inspector's Serves does not lead to the intent"; exit 1; }

# ---------------------------------------------------------------- unknown or bad
get '/ui/roadmap?intent=nope' | grep -q 'nope is not an intent of this repository' \
  || { echo "    an id the repository does not hold is not said to be unknown"; exit 1; }
page="$(get '/ui/roadmap?intent=probe%22%20onclick%3D%22x')"
printf '%s' "$page" | grep -q 'id="intent"' && { echo "    a value that is no id opened an inspector"; exit 1; }
printf '%s' "$page" | grep -q 'onclick' && { echo "    the page echoes a handler a link carried"; exit 1; }

echo "    an intent opens with its criteria, and leads to the milestone and the issue serving it"

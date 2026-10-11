# majordomus-covers: none
# The Governance screen's rule inspector (I2478), through a real server over a fixture holding
# the vendored rules `majordomus init` seeds:
#
#   a rule          ?rule= opens it with what it states and the tests that prove it
#   the way in      the Governance screen's rows lead to the inspector, by the rule's own id
#   unknown or bad  a rule the index does not hold is said as the refusal it is; a value that
#                   is no id is never asked
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

# a rule the fixture holds, read from the same report the screen reads
id="$(get /api/v1/rules | jq -r '[.rules[].rule | if type == "object" then .id else . end] | map(select(. != null and . != "")) | .[0]')"
[ -n "$id" ] && [ "$id" != null ] || { echo "    the fixture holds no rule to inspect"; exit 1; }

# ---------------------------------------------------------------- a rule
page="$(get "/ui/governance?rule=$id")"
printf '%s' "$page" | grep -q 'id="rule"' \
  || { echo "    ?rule=$id opened no inspector:"; printf '%s\n' "$page" | grep -o 'mj-ui-empty">[^<]*' | sed 's/^/    | /'; echo; exit 1; }
printf '%s' "$page" | grep -q 'What proves it' || { echo "    the inspector does not say what proves it"; exit 1; }

# ---------------------------------------------------------------- the way in
get /ui/governance | grep -qF "href=\"/ui/governance?rule=$id#rule\"" \
  || { echo "    the Governance rows do not lead to $id's inspector"; exit 1; }

# ---------------------------------------------------------------- unknown or bad
page="$(get '/ui/governance?rule=no.such-rule')"
printf '%s' "$page" | grep -q 'id="rule"' && { echo "    an unknown rule opened an inspector"; exit 1; }
printf '%s' "$page" | grep -q 'no.such-rule' || { echo "    an unknown rule is not said as its refusal"; exit 1; }
page="$(get '/ui/governance?rule=x%22%20onclick%3D%22y')"
printf '%s' "$page" | grep -q 'onclick' && { echo "    the page echoes a handler a link carried"; exit 1; }

echo "    a rule opens with what proves it, and every row of the Governance screen leads to one"

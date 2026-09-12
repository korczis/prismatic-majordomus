# majordomus-covers: none
# The typed session domain, through the real executable, against a repository this case
# makes rather than against the one in the checkout (ADR 0047).
#
# What is proved here:
#
#  * the lifecycle machine is one declaration and reaches every projection — the capability
#    registry, MCP and HTTP all answer the same states and transitions, and none of them
#    carries a list of its own;
#  * the machine has exactly one terminal state, and nothing leaves it. Episode
#    s-20260909152316-024f has four immutable records in this repository because the
#    previous representation — a file that exists or does not — could not say that;
#  * the machine depends on no task. The signature of the transition is what ADR 0052's
#    six-day outage turned on, and this asserts the projection says so out loud rather
#    than leaving a reader to infer an absence;
#  * the identities the stores conflate are reported as separate subjects with each store's
#    own spelling beside the canonical value: a repository that is a path in one half and a
#    remote URL in the other, a session that is an episode id and a provider's own string;
#  * a provider session is reported as an external correlation id and never as the session
#    identity, in the answer itself and not only in prose;
#  * and the domain is additive: nothing it adds writes, and the two capabilities it
#    declares are read-only in the registry's own judgement.
#
# Nothing here asserts a count of capabilities, a route table or a list of module ids:
# every expectation is about the shape of the answer, so the executable can grow without
# this file being edited.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    skip: jq not installed"; exit 0; }
command -v curl >/dev/null 2>&1 || { echo "    skip: no curl"; exit 0; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj240.XXXXXX")"
SRV=""; trap 'rm -rf "$S"; [ -n "$SRV" ] && kill "$SRV" 2>/dev/null' EXIT

serve_up() {
  "$RB" serve --repo "$PWD" --port 0 > "$S/out.txt" 2> "$S/err.txt" & SRV=$!
  i=0
  until grep -q 'listening on http://' "$S/err.txt" 2>/dev/null; do
    i=$((i+1)); [ "$i" -lt 300 ] || { echo "    the server never listened"; cat "$S/err.txt"; return 1; }
    kill -0 "$SRV" 2>/dev/null || { echo "    the server exited before listening"; cat "$S/err.txt"; return 1; }
    sleep 0.1
  done
  U="$(sed -n 's#.*listening on \(http://127\.0\.0\.1:[0-9]*\).*#\1#p' "$S/err.txt" | head -n 1)"
  [ -n "$U" ] || { echo "    no URL on the listening line"; cat "$S/err.txt"; return 1; }
}
serve_down() {
  [ -n "$SRV" ] || return 0
  kill "$SRV" 2>/dev/null || true
  wait "$SRV" 2>/dev/null || true
  SRV=""
}

# --- a repository of the layer, with an open episode of its own
"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

mkdir -p .ai/local/state/sessions-open
cat > .ai/local/state/sessions-open/01Bv2gJsf.yaml <<'YAML'
session_id: s-20260910205542-e2a6
started_at: 2026-09-10T20:55:42Z
owner: "korczis"
provider: "claude-code"
provider_session: "01Bv2gJsf"
branch: master
start_head: 9f3a2a0
YAML

# ---------------------------------------------------------------- the declaration
# One canonical declaration, and the registry is what reports its projections. A capability
# that lost its MCP tool or its route would still compile and still answer; this is what
# would not.
run_quiet "$S/desc.err" "$RB" capabilities describe session_domain.machine --format json > "$S/machine.json"
jq -e '.kind == "query"' "$S/machine.json" >/dev/null \
  || { echo "    session_domain.machine is not declared read-only"; exit 1; }
run_quiet "$S/desc2.err" "$RB" capabilities describe session_domain.identity --format json > "$S/identity.json"
jq -e '.kind == "query"' "$S/identity.json" >/dev/null \
  || { echo "    session_domain.identity is not declared read-only"; exit 1; }

# The domain's write path reaches no surface: every capability of the module reads. This is
# the assertion that fails the day somebody exposes the closer over HTTP, which would offer
# a write of this checkout's records to a caller that does not share the checkout.
run_quiet "$S/list.err" "$RB" capabilities list --format json > "$S/all.json"
jq -e '[.capabilities[] | select(.id | startswith("session_domain.")) | select(.kind != "query")] | length == 0' \
  "$S/all.json" >/dev/null \
  || { echo "    a session_domain.* capability is not read-only"; exit 1; }

# ---------------------------------------------------------------- the machine, over HTTP
serve_up || exit 1

curl -fsS "$U/api/v1/session/machine" > "$S/m.json" \
  || { echo "    GET /api/v1/session/machine failed"; exit 1; }

# exactly one terminal state, and nothing leaves it. The four records of
# s-20260909152316-024f exist because nothing could previously say this.
jq -e '[.states[] | select(.terminal)] | length == 1' "$S/m.json" >/dev/null \
  || { echo "    the machine does not have exactly one terminal state"; exit 1; }
jq -e '[.states[] | select(.terminal) | .may_move_to[]] | length == 0' "$S/m.json" >/dev/null \
  || { echo "    the terminal state has a move out of it"; exit 1; }
jq -e '.states[] | select(.state == "closed") | .terminal' "$S/m.json" >/dev/null \
  || { echo "    the closed state is not the terminal one"; exit 1; }

# every transition's move is one the machine allows: the table and the vocabulary are
# derived from the same types, so they cannot disagree — this is what proves they are.
jq -e '
  (reduce .states[] as $s ({}; . + { ($s.state): $s.may_move_to })) as $allow
  | [ .transitions[] | . as $t | select((($allow[$t.from] // []) | index($t.to)) == null) ] | length == 0
' "$S/m.json" >/dev/null \
  || { echo "    a transition runs a move the machine forbids"; exit 1; }

# the six the audit's evidence requires, and no fewer
for t in open resume checkpoint detach close recover; do
  jq -e --arg t "$t" '[.transitions[] | select(.transition == $t)] | length == 1' "$S/m.json" >/dev/null \
    || { echo "    the machine has no $t transition"; exit 1; }
done

# a checkpoint is an event inside a state and is a transition anyway: an event modelled as
# "nothing happened" cannot be counted, and counting it is how a stopped writer is seen
jq -e '.transitions[] | select(.transition == "checkpoint") | .moves_state == false' "$S/m.json" >/dev/null \
  || { echo "    a checkpoint is not reported as an event inside a state"; exit 1; }

# and the machine says out loud what it does not depend on. ADR 0052's whole finding.
jq -e '[.independent_of[] | select(test("task"))] | length >= 1' "$S/m.json" >/dev/null \
  || { echo "    the machine does not state its independence from task state"; exit 1; }
jq -e '.decision | test("0052")' "$S/m.json" >/dev/null \
  || { echo "    the machine does not cite the decision it implements"; exit 1; }

# ---------------------------------------------------------------- the identities
curl -fsS "$U/api/v1/session/identity" > "$S/i.json" \
  || { echo "    GET /api/v1/session/identity failed"; exit 1; }

for subject in repository checkout episode provider_session; do
  jq -e --arg s "$subject" '[.facets[] | select(.subject == $s)] | length == 1' "$S/i.json" >/dev/null \
    || { echo "    no facet for $subject"; exit 1; }
done

# the checkout always has a canonical identity: the question has to be answerable for a
# record written on a disk that has since been unmounted
jq -e '.facets[] | select(.subject == "checkout") | (.canonical | length) == 32' "$S/i.json" >/dev/null \
  || { echo "    the checkout has no canonical identity"; exit 1; }

# the repository is spelled twice, by two writers, in two halves — which is the conflation
jq -e '.facets[] | select(.subject == "repository") | ([.spellings[].spelling] | sort | unique | length) >= 2' \
  "$S/i.json" >/dev/null \
  || { echo "    the repository is not reported with both of its spellings"; exit 1; }

# the open episode is reported by its own id, and the provider session beside it and never
# as it: an external correlation id, said in the answer and not only in prose
jq -e '.facets[] | select(.subject == "episode") | .canonical == "s-20260910205542-e2a6"' "$S/i.json" >/dev/null \
  || { echo "    the open episode is not reported by its own id"; exit 1; }
# `canonical` is omitted when empty, so the absence and the empty string are one answer
jq -e '.facets[] | select(.subject == "provider_session") | (.canonical // "") == ""' "$S/i.json" >/dev/null \
  || { echo "    a provider session was reported as a canonical identity"; exit 1; }
jq -e '.facets[] | select(.subject == "provider_session") | [.spellings[].value] | index("01Bv2gJsf") != null' \
  "$S/i.json" >/dev/null \
  || { echo "    the provider's own session string is not reported as a spelling"; exit 1; }
jq -e '.facets[] | select(.subject == "provider_session") | .note | test("correlation")' "$S/i.json" >/dev/null \
  || { echo "    the provider session is not named as a correlation id"; exit 1; }

# the standing finding: comparing the two halves' spellings answers 'different repository'
# for two records of one, and the surface says so rather than leaving a reader to find out
jq -e '[.findings[] | select(test("remote URL"))] | length >= 1' "$S/i.json" >/dev/null \
  || { echo "    the conflation is not reported as a finding"; exit 1; }

# ---------------------------------------------------------------- one declaration, every projection
# The transports are not asked the same question twice here: the crate's own property
# harness (apps/majordomus-cli/tests/properties.rs) already proves that every capability
# answers identically over the executor, MCP and HTTP, for every capability rather than for
# the two this case is about. What is asserted here is the thing that harness cannot see —
# that these two capabilities *claim* those projections at all, from the one declaration.
jq -e '.exposure.mcp.tool == "majordomus_session_machine"' "$S/machine.json" >/dev/null \
  || { echo "    session_domain.machine declares no MCP tool"; exit 1; }
jq -e '.exposure.mcp.resource.uri == "majordomus://session/machine"' "$S/machine.json" >/dev/null \
  || { echo "    session_domain.machine declares no MCP resource"; exit 1; }
jq -e '.exposure.http.path == "/api/v1/session/machine"' "$S/machine.json" >/dev/null \
  || { echo "    session_domain.machine declares no route"; exit 1; }
jq -e '.exposure.mcp.tool == "majordomus_session_identity"' "$S/identity.json" >/dev/null \
  || { echo "    session_domain.identity declares no MCP tool"; exit 1; }
jq -e '.exposure.http.path == "/api/v1/session/identity"' "$S/identity.json" >/dev/null \
  || { echo "    session_domain.identity declares no route"; exit 1; }

# and the routes the declaration claims are the routes the running server actually serves,
# which is what makes the claim above a measurement rather than a restatement of itself
[ "$(curl -s -o /dev/null -w '%{http_code}' "$U/api/v1/session/machine")" = 200 ] \
  || { echo "    the declared route for session_domain.machine is not served"; exit 1; }
[ "$(curl -s -o /dev/null -w '%{http_code}' "$U/api/v1/session/identity")" = 200 ] \
  || { echo "    the declared route for session_domain.identity is not served"; exit 1; }

serve_down

# ---------------------------------------------------------------- additive
# Nothing this domain adds wrote anything. The shell tool still owns the lifecycle, and the
# open record this case planted is exactly as it was planted (ADR 0047).
[ -d .ai/local/state/sessions-closing ] \
  && { echo "    the read path created a claim directory"; exit 1; }
grep -q '^session_id: s-20260910205542-e2a6$' .ai/local/state/sessions-open/01Bv2gJsf.yaml \
  || { echo "    the open record was modified by a read"; exit 1; }
# the skeleton's own README lives there; a *record* does not
n="$(find .ai/repo/sessions -name '*.md' ! -name 'README.md' 2>/dev/null | wc -l | tr -d ' ')"
[ "$n" = 0 ] || { echo "    $n session record(s) were written by a read"; exit 1; }

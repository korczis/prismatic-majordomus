# claims: advisor-failure-is-recorded-and-recovers
# A broken advisor neither blocks the work nor stays broken forever (ADR 0098). The
# preferred advisor times out, the other is not installed: the timeout is recorded with its
# diagnostic, the session concludes on local evidence, validates, and the task's reasoning
# ends validated — complete, not failed. A second timeout opens the advisor's circuit, so
# the next plan leaves it out and says why; after the cooldown it is available again and
# marked recovering, and its next answer closes the circuit. A rate limit, an
# authentication failure and a malformed answer are each recorded as what they are, and a
# malformed answer says nothing about the advisor's standing.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "no jq"
reasoning_fixture
rz_stub fake-alpha
cat > "$R/share/advisors.yaml" <<'YAML'
version: 1
capabilities:
  - id: independent_reasoning
    description: x
advisors:
  - id: alpha
    title: Alpha
    transport: cli
    adapter: fixture-scripted
    executable: fake-alpha
    capabilities: [independent_reasoning]
  - id: beta
    title: Beta
    transport: cli
    adapter: fixture-scripted
    executable: fake-beta
    capabilities: [independent_reasoning]
YAML
assess() { rz_record "{\"kind\":\"assessment\",\"subject\":\"$1\",\"materiality\":\"material\",\"evidence\":[{\"kind\":\"file\",\"reference\":\"src/a.rs\"}]}"; }

# ---------------------------------------------------------------- 1. a timeout does not stop the work
RZ_ENV='MAJORDOMUS_FIXTURE_ANSWERS={"alpha":{"hang":true}}'
A="$(assess "first question")"
expect_exit 0 rz_consult --assessment "$A" --timeout 1
expect_grep "advisor    alpha"
expect_grep "status     timeout: no answer within 1000 ms"
expect_grep "next       continue on local evidence"
expect_no_grep "advisor    beta"
K="$(rz_record "{\"kind\":\"conclusion\",\"assessment\":\"$A\",\"decision\":\"decided locally\",\"rationale\":\"the local evidence suffices\",\"evidence\":[{\"kind\":\"file\",\"reference\":\"src/a.rs\"}],\"validation_plan\":[\"run the unit\"]}")"
rz_record "{\"kind\":\"validation\",\"conclusion\":\"$K\",\"check\":\"the unit passes\",\"outcome\":\"pass\"}" >/dev/null
S="$(rz_json reasoning status)"
[ "$(printf '%s' "$S" | jq -r --arg a "$A" '.state.assessments[] | select(.id==$a) | .standing')" = validated ] \
  || { echo "    the work did not complete after the advisor timed out"; printf '%s\n' "$S"; exit 1; }
[ "$(printf '%s' "$S" | jq -r '.state.totals.consultations_failed')" = 1 ] || { echo "    the timeout was not recorded"; exit 1; }
printf '%s' "$S" | jq -e '.state.consultations[0].diagnostic | test("no answer")' >/dev/null || { echo "    the timeout carries no diagnostic"; exit 1; }
# a review that timed out is not a review
expect_exit 1 rz_record "{\"kind\":\"conclusion\",\"assessment\":\"$A\",\"supersedes\":\"$K\",\"decision\":\"x\",\"rationale\":\"r\",\"evidence\":[{\"kind\":\"file\",\"reference\":\"f\"}],\"validation_plan\":[\"v\"],\"consultations\":[\"$(printf '%s' "$S" | jq -r '.state.consultations[0].id')\"]}"

# ---------------------------------------------------------------- 2. a second timeout opens the circuit
B="$(assess "second question")"
expect_exit 0 rz_consult --assessment "$B" --timeout 1
expect_exit 0 rz reasoning advisors
expect_grep "alpha +temporarily_failed +cli +consecutive_failures until"
expect_grep "beta +unavailable +cli +executable_not_found"
expect_exit 0 rz reasoning plan --materiality material
expect_grep "outcome     decide_locally"
expect_grep "excluded    alpha — temporarily_failed: consecutive_failures"

# ---------------------------------------------------------------- 3. after the cooldown it recovers
sleep 2
RZ_ENV='MAJORDOMUS_REASONING_COOLDOWN_SECONDS=1 MAJORDOMUS_FIXTURE_ANSWERS={"alpha":{"answer":{"conclusion":"fine","stance":"supports"}}}'
expect_exit 0 rz reasoning advisors
expect_grep "alpha +available +cli +present \(recovering\)"
C="$(assess "third question")"
expect_exit 0 rz_consult --assessment "$C"
expect_grep "conclusion fine"
expect_exit 0 rz reasoning advisors
expect_grep "alpha +available +cli +present$"

# ---------------------------------------------------------------- 4. each failure is what it is
for case in 'rate_limited:{"fail":"rate_limited","retryAfter":30}:rate_limited +cli +rate_limited' \
            'malformed:{"text":"I_think_so."}:available +cli +present' \
            'auth_failed:{"fail":"auth_failed"}:temporarily_failed +cli +authentication_failed'; do
  want="${case%%:*}"; rest="${case#*:}"; script="${rest%:*}"; standing="${rest##*:}"
  RZ_ENV="MAJORDOMUS_FIXTURE_ANSWERS={\"alpha\":$script}"
  X="$(assess "question $want")"
  expect_exit 0 rz_consult --assessment "$X" --timeout 5
  expect_grep "status     $want"
  RZ_ENV=""
  expect_exit 0 rz reasoning advisors
  expect_grep "alpha +$standing"
  # each one is followed by an answer, so the next starts from a closed circuit
  RZ_ENV='MAJORDOMUS_REASONING_COOLDOWN_SECONDS=0 MAJORDOMUS_FIXTURE_ANSWERS={"alpha":{"answer":{"conclusion":"ok","stance":"supports"}}}'
  Y="$(assess "reset after $want")"
  expect_exit 0 rz_consult --assessment "$Y"
  RZ_ENV=""
done

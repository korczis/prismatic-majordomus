# claims: reasoning-survives-the-session
# A consultation is paid for once (ADR 0098). Session A gathers evidence, consults an
# advisor, concludes, implements part of the work and hands over. Session B — a new process
# with nothing but the checkout — finds the decision, who reviewed it and the evidence in
# `majordomus context` and in the handover, and when it meets the same question again the
# plan reuses the standing conclusion: no advisor is asked (the scripted advisor would hang
# if it were). B continues and validates. New evidence reopens the question, and only then
# is the advisor asked again, and the new conclusion supersedes the old one.
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
YAML
mj() { ( cd "$R" && rz_env MAJORDOMUS_BIN="$RB" $RZ_ENV "$R/bin/majordomus" "$@" ); }
mj start "move the transaction boundary" --scope src >/dev/null 2>&1 || { echo "    the task did not start"; exit 1; }
SUBJECT='Where is the transaction boundary of the order service?'
assessment() { printf '{"kind":"assessment","subject":"%s","materiality":"high","evidence":[{"kind":"file","reference":"src/order.rs:40"}]%s}' "$SUBJECT" "$1"; }

# ---------------------------------------------------------------- session A
RZ_ENV='MAJORDOMUS_FIXTURE_ANSWERS={"alpha":{"answer":{"conclusion":"keep_it_at_the_service_layer","stance":"supports"}}}'
A="$(rz_record "$(assessment '')")"
expect_exit 0 rz_consult --assessment "$A"
expect_grep "conclusion keep_it_at_the_service_layer"
c="$(rz_json reasoning status | jq -r '.state.consultations[0].id')"
K="$(rz_record "{\"kind\":\"conclusion\",\"assessment\":\"$A\",\"decision\":\"keep the transaction boundary at the service layer\",\"rationale\":\"every write path enters through the service\",\"evidence\":[{\"kind\":\"file\",\"reference\":\"src/order.rs:40\"}],\"consultations\":[\"$c\"],\"validation_plan\":[\"a failure-path integration test\"]}")"
mkdir -p "$R/src"; printf 'fn place() { tx(|| { /* half done */ }) }\n' > "$R/src/order.rs"
mj handover --derive >/dev/null 2>&1 || { echo "    session A could not hand over"; exit 1; }
RZ_ENV=""

# ---------------------------------------------------------------- session B: what it is told
expect_exit 0 mj context
expect_grep "^## REASONING"
expect_grep "Decision $K: keep the transaction boundary at the service layer — reviewed by alpha"
expect_exit 0 mj handover --resolve
expect_grep "^# Reasoning"
expect_grep "reviewed by alpha"

# ---------------------------------------------------------------- session B: the same question
RZ_ENV='MAJORDOMUS_FIXTURE_ANSWERS={"alpha":{"hang":true}}'
B="$(rz_record "$(assessment '')")"
expect_exit 0 rz_consult --assessment "$B" --timeout 2
expect_grep "outcome    reuse_prior"
expect_grep "conclusion $K already answers this subject"
expect_no_grep "\[reasoning\] consultation"
[ "$(rz_json reasoning status | jq -r '.state.totals.consultations_completed + .state.totals.consultations_failed')" = 1 ] \
  || { echo "    session B asked an advisor again"; exit 1; }
# B continues the work and validates the standing decision
printf 'fn place() { tx(|| { reserve(); charge(); }) }\n' > "$R/src/order.rs"
rz_record "{\"kind\":\"validation\",\"conclusion\":\"$K\",\"check\":\"the failure-path test rolls back both writes\",\"outcome\":\"pass\"}" >/dev/null
expect_exit 0 rz reasoning status --report
expect_grep "validation 1 passed"

# ---------------------------------------------------------------- new evidence reopens it
RZ_ENV='MAJORDOMUS_FIXTURE_ANSWERS={"alpha":{"answer":{"conclusion":"move_it_to_the_repository","stance":"opposes"}}}'
N="$(rz_record "$(assessment ',"new_evidence":true')")"
expect_exit 0 rz_consult --assessment "$N"
expect_grep "outcome    consult"
expect_grep "conclusion move_it_to_the_repository"
c2="$(rz_json reasoning status | jq -r '.state.consultations[-1].id')"
K2="$(rz_record "{\"kind\":\"conclusion\",\"assessment\":\"$N\",\"supersedes\":\"$K\",\"decision\":\"move the boundary to the repository\",\"rationale\":\"a batch path bypasses the service\",\"evidence\":[{\"kind\":\"file\",\"reference\":\"src/batch.rs:12\"}],\"consultations\":[\"$c2\"],\"validation_plan\":[\"the batch path test\"]}")"
S="$(rz_json reasoning status)"
[ "$(printf '%s' "$S" | jq -r --arg k "$K" '.state.conclusions[] | select(.id==$k) | .superseded_by')" = "$K2" ] \
  || { echo "    the old conclusion is not marked superseded"; exit 1; }
expect_exit 0 rz reasoning status --report
expect_grep "Decision $K2"
expect_no_grep "Decision $K:"

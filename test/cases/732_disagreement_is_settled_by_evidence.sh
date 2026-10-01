# claims: advice-is-weighed-not-counted
# Advisors disagree, and a count does not decide (ADR 0098). Two scripted advisors are
# consulted through the real transport: one supports the executor's hypothesis A, the
# other proposes B. Two positions of three say A. The writer refuses a conclusion while the
# disagreement is unsettled, refuses a "resolution" that cites no evidence, and admits the
# conclusion B — the minority position — once an experiment settles it. The conclusion
# weighs both answers, says which it rejected, and its review count is computed.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "no jq"
reasoning_fixture
rz_stub fake-alpha
rz_stub fake-beta
cat > "$R/share/advisors.yaml" <<'YAML'
version: 1
capabilities:
  - id: independent_reasoning
    description: x
  - id: code_review
    description: y
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
    capabilities: [independent_reasoning, code_review]
YAML
RZ_ENV='MAJORDOMUS_FIXTURE_ANSWERS={"alpha":{"answer":{"conclusion":"A:_a_typed_domain_error","stance":"supports","confidence":"high"}},"beta":{"answer":{"conclusion":"B:_exit_and_let_the_supervisor_restart","stance":"opposes","falsifiers":["a_caller_that_retries"]}}}'

A="$(rz_record '{"kind":"assessment","subject":"timeout semantics of the worker","materiality":"high","hypothesis":"A: a typed domain error","capabilities":["independent_reasoning","code_review"],"evidence":[{"kind":"file","reference":"src/worker.rs:10"}]}')"
expect_exit 0 rz_consult --assessment "$A"
expect_grep "outcome    consult"
expect_grep "advisor    alpha"
expect_grep "advisor    beta"
expect_grep "stance     supports"
expect_grep "stance     opposes"

S="$(rz_json reasoning status)"
c_alpha="$(printf '%s' "$S" | jq -r '.state.consultations[] | select(.advisor=="alpha") | .id')"
c_beta="$(printf '%s' "$S" | jq -r '.state.consultations[] | select(.advisor=="beta") | .id')"
[ -n "$c_alpha" ] && [ -n "$c_beta" ] || { echo "    both consultations should be recorded"; printf '%s\n' "$S"; exit 1; }

D="$(rz_record "{\"kind\":\"disagreement\",\"assessment\":\"$A\",\"positions\":[{\"source\":\"primary\",\"position\":\"A\"},{\"source\":\"$c_alpha\",\"position\":\"A\"},{\"source\":\"$c_beta\",\"position\":\"B\"}],\"divergent_assumptions\":[\"is a timeout an infrastructure failure\"],\"discriminating_question\":\"does any caller retry on the domain error?\"}")"

# two positions of three say A, and that decides nothing
base="\"assessment\":\"$A\",\"rationale\":\"r\",\"evidence\":[{\"kind\":\"test\",\"reference\":\"test/supervision\"}],\"consultations\":[\"$c_alpha\",\"$c_beta\"],\"disagreements\":[\"$D\"],\"validation_plan\":[\"run the supervision test\"]"
expect_exit 1 rz_record "{\"kind\":\"conclusion\",\"decision\":\"A\",$base}"
grep -q 'unresolved' "$T/rz_record.err" || { echo "    the refusal does not name the unresolved disagreement"; cat "$T/rz_record.err"; exit 1; }
# a vote is not a resolution
expect_exit 1 rz_record "{\"kind\":\"resolution\",\"disagreement\":\"$D\",\"experiment\":\"count\",\"outcome\":\"2 to 1\",\"favours\":\"primary\",\"evidence\":[]}"
grep -q 'never by a count' "$T/rz_record.err" || { echo "    a resolution without evidence was not refused for it"; exit 1; }
# the experiment favours the minority
rz_record "{\"kind\":\"resolution\",\"disagreement\":\"$D\",\"experiment\":\"run the supervision failure test\",\"outcome\":\"no caller retries; the restart is the recovery\",\"favours\":\"$c_beta\",\"evidence\":[{\"kind\":\"test\",\"reference\":\"test/supervision\",\"note\":\"passed\"}]}" >/dev/null
# the conclusion must weigh every answer: leaving one out is refused
expect_exit 1 rz_record "{\"kind\":\"conclusion\",\"decision\":\"B\",\"assessment\":\"$A\",\"rationale\":\"r\",\"evidence\":[{\"kind\":\"test\",\"reference\":\"t\"}],\"consultations\":[\"$c_beta\"],\"disagreements\":[\"$D\"],\"validation_plan\":[\"v\"]}"
K="$(rz_record "{\"kind\":\"conclusion\",\"decision\":\"B: exit and let the supervisor restart\",\"rejected\":[\"A: no caller handles the domain error\"],$base}")"
C="$(rz_json reasoning explain "$K")"
[ "$(printf '%s' "$C" | jq -r '.record.body.decision')" = "B: exit and let the supervisor restart" ] || { echo "    the decision is not B"; exit 1; }
[ "$(printf '%s' "$C" | jq -r '.record.body.independent_review_count')" = 2 ] || { echo "    the review count is not computed from the two answers"; exit 1; }
printf '%s' "$C" | jq -e '.chain[] | select(.phase=="experiment")' >/dev/null || { echo "    the experiment is missing from the provenance"; exit 1; }

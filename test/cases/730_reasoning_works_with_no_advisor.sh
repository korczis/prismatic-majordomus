# majordomus-covers: doctor
# claims: reasoning-works-with-no-advisor
# Majordomus reasons and finishes the work with no advisor at all (ADR 0098). Every
# advisor of the shipped catalogue is absent here — no executable on PATH, no credential in
# the environment — on any machine, including one where all of them are installed, because
# the case runs in an environment it built from nothing.
#
# The workflow is the whole one, not boot: a task starts, the advisors report themselves
# absent as ordinary state, a material uncertainty is assessed with its evidence, the plan
# is a structured local review, the transport consults nobody and exits cleanly, a
# conclusion is formed on local evidence with zero independent review claimed, validation
# runs, the derived handover carries the decision, and the state, the report and doctor all
# say the same thing. Then the same with each operating mode that admits nothing.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "no jq"
reasoning_fixture

# ---------------------------------------------------------------- 1. absence is state
expect_exit 0 rz reasoning advisors
expect_grep "reasoning   operational"
expect_grep "0 available · 5 optional unavailable"
for advisor in chatgpt gemini codex ollama lmstudio; do expect_grep "^  $advisor "; done
expect_grep "chatgpt +not_configured +api +credential_absent"
expect_grep "codex +unavailable +cli +executable_not_found"
expect_grep "independent_reasoning +local review"
J="$(rz_json reasoning advisors)"
[ "$(printf '%s' "$J" | jq -r '.operational')" = true ] || { echo "    reasoning is not operational without advisors"; exit 1; }
[ "$(printf '%s' "$J" | jq -r '.available')" = 0 ] || { echo "    an advisor was reported available"; exit 1; }

# the environment snapshot says the same, from the same derivation
J="$(rz_json env)"
[ "$(printf '%s' "$J" | jq -r '.reasoning.operational')" = true ] || { echo "    env: reasoning not operational"; exit 1; }
[ "$(printf '%s' "$J" | jq -r '.reasoning.available | length')" = 0 ] || { echo "    env: an advisor reported available"; exit 1; }

# ---------------------------------------------------------------- 2. a task with a material uncertainty
( cd "$R" && rz_env "$R/bin/majordomus" start "decide the timeout semantics" --scope src >/dev/null 2>&1 ) \
  || { echo "    the task did not start"; exit 1; }
task="$(sed -n 's/^id: //p' "$R/.ai/local/state/current.yaml")"
[ -n "$task" ] || { echo "    no task id was recorded"; exit 1; }

# evidence before opinion: a material assessment without evidence is refused
expect_exit 1 rz_record '{"kind":"assessment","subject":"timeout semantics","materiality":"high"}'
A="$(rz_record '{"kind":"assessment","subject":"Is a worker timeout a typed domain error or a crash?","materiality":"high","confidence":"medium","hypothesis":"a typed domain error","facts":["callers match on the error type"],"questions":["does any caller retry?"],"evidence":[{"kind":"file","reference":"src/worker.rs:10"},{"kind":"test","reference":"test/worker_timeout"}]}')"

# ---------------------------------------------------------------- 3. the plan is a local review
expect_exit 0 rz reasoning plan --materiality high
expect_grep "outcome     decide_locally"
expect_grep "no available advisor offers independent_reasoning"
expect_grep "local       name what observation would falsify the hypothesis"

# the transport consults nobody, records the plan, and exits 0: no advisor is not an error
expect_exit 0 rz_consult --assessment "$A"
expect_grep "outcome    decide_locally"
expect_grep "local      state the hypothesis"
expect_no_grep "\[reasoning\] consultation"
P="$(ls "$R/.ai/local/state/reasoning/$task" | sed -n 's/^\(plan-.*\)\.json$/\1/p' | head -n 1)"
[ -n "$P" ] || { echo "    the plan was not recorded"; exit 1; }
[ "$(jq -r '.body.plan.selected | length' "$R/.ai/local/state/reasoning/$task/$P.json")" = 0 ] \
  || { echo "    the recorded plan selected an advisor that is not there"; exit 1; }

# no fabricated review: a consultation of an advisor the plan did not select is refused
expect_exit 1 rz_record "{\"kind\":\"consultation\",\"plan\":\"$P\",\"advisor\":\"chatgpt\",\"status\":\"completed\",\"conclusion\":\"agreed\",\"stance\":\"supports\"}"

# ---------------------------------------------------------------- 4. conclude, implement, validate
expect_exit 1 rz_record "{\"kind\":\"conclusion\",\"assessment\":\"$A\",\"decision\":\"typed error\",\"rationale\":\"callers match on it\",\"evidence\":[{\"kind\":\"file\",\"reference\":\"src/worker.rs:10\"}]}"
K="$(rz_record "{\"kind\":\"conclusion\",\"assessment\":\"$A\",\"decision\":\"represent the timeout as a typed domain error\",\"rationale\":\"every caller matches on the error type and none retries\",\"evidence\":[{\"kind\":\"file\",\"reference\":\"src/worker.rs:10\"}],\"rejected\":[\"crash and restart: no caller expects a restart\"],\"validation_plan\":[\"a timeout test returns the typed error\"],\"confidence\":\"high\"}")"
[ "$(jq -r '.body.independent_review_count' "$R/.ai/local/state/reasoning/$task/$K.json")" = 0 ] \
  || { echo "    a local conclusion claims independent review"; exit 1; }

# the implementation the conclusion calls for, and its validation
mkdir -p "$R/src"; printf 'timeout -> TypedError::Timeout\n' > "$R/src/worker.rs"
grep -q TypedError "$R/src/worker.rs" || { echo "    the implementation step did not happen"; exit 1; }
rz_record "{\"kind\":\"validation\",\"conclusion\":\"$K\",\"check\":\"the worker maps a timeout to the typed error\",\"command\":\"grep TypedError src/worker.rs\",\"outcome\":\"pass\"}" >/dev/null

# ---------------------------------------------------------------- 5. state, report, handover
J="$(rz_json reasoning status)"
[ "$(printf '%s' "$J" | jq -r '.state.assessments[0].standing')" = validated ] || { echo "    the assessment is not validated:"; printf '%s\n' "$J"; exit 1; }
[ "$(printf '%s' "$J" | jq -r '.state.totals.independent_reviewers')" = 0 ] || { echo "    the state claims a reviewer"; exit 1; }
expect_exit 0 rz reasoning status --report
expect_grep "reviewed by no independent advisor \(local reasoning\); validation 1 passed"
expect_exit 0 rz reasoning explain "$K"
expect_grep "local +plan-"
expect_grep "conclusion +$K"
expect_grep "validation +validation-"

( cd "$R" && rz_env MAJORDOMUS_BIN="$RB" "$R/bin/majordomus" handover --derive >/dev/null 2>&1 ) \
  || { echo "    the derived handover was refused"; exit 1; }
h="$(ls -t "$R/.ai/local/state/handovers/"*.md | head -n 1)"
grep -q '^# Reasoning' "$h" || { echo "    the handover has no Reasoning section"; cat "$h"; exit 1; }
grep -q "represent the timeout as a typed domain error" "$h" || { echo "    the handover does not carry the decision"; cat "$h"; exit 1; }
grep -q "local reasoning" "$h" || { echo "    the handover does not say the decision was local"; exit 1; }

# ---------------------------------------------------------------- 6. every mode that admits nothing
for mode in ci offline; do
  RZ_ENV="MAJORDOMUS_REASONING_MODE=$mode" expect_exit 0 rz reasoning plan --materiality critical
  expect_grep "outcome     decide_locally"
done
# a CI runner is the ci mode, whatever is installed
rz_stub codex
RZ_ENV="CI=true" expect_exit 0 rz reasoning advisors
expect_grep "mode ci \(CI\)"
expect_grep "codex +disabled +cli +mode_ci"

# ---------------------------------------------------------------- 7. absence is never a failure
expect_exit 0 rz reasoning check
expect_grep "no finding"

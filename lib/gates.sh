#!/usr/bin/env bash
# shellcheck disable=SC2034  # MJ_DOCTRINE_SKIPPED is read by the dispatcher in doctrine.sh
# gates — the validation gates a task's change set must pass, recorded and enforced.
#
# `evidence --covers` records that an *obligation* was discharged; this records that a
# *gate* reported. The two are different promises: an obligation is what a worker owes, a
# gate is what the validation pipeline says about the tree. Until this file the second had
# no bearing on whether a task could be finished — a task could be completed with
# `rust-check` red, because a gate was a property of a pull request and not of a task.
#
# Three things live here and nothing else:
#
#   1. a reader of the CI model, `.ai/repo/ci/gates.yaml`, for the two questions recording
#      needs: does this gate exist, and which files is a run of it evidence over;
#   2. the recording itself, as a `task.gate` ledger line carrying the hash of those files,
#      so that changing one of them makes the run stale rather than merely old;
#   3. the finish/check validator, which asks the Rust executable for the judgement rather
#      than making one of its own.
#
# The judgement is deliberately not here. `gates.completion` in the Rust executable decides
# what each gate's status is, and every surface — this validator, the HTTP API, MCP, the
# Cockpit — reads that one execution. A judgement with two implementations is how `check`
# and a gate come to disagree about whether the same work is finished, which is the failure
# ADR 38 names; the obligation half accepted a second implementation and said so, and this
# half does not.
#
# What *is* here that the Rust also derives is `mj_gate_inputs`: which pathspecs a gate's
# evidence is taken over. It has to be, because recording happens where the ledger is
# written and the ledger has one writer. The two derivations are proved equal by the
# cheapest possible check — a run recorded here reads back as `pass` and not as `stale` —
# which `test/cases/131_completion_gates.sh` asserts.

MJ_GATE_FLAT=""

# The CI model, flattened once per process. Absent is not an error: a repository with no CI
# model is one whose gates cannot be known, which every reader reports as `unknown`.
mj_gate_model_load() {
  [ -n "$MJ_GATE_FLAT" ] && [ -f "$MJ_GATE_FLAT" ] && return 0
  local model="$MJ_ROOT/.ai/repo/ci/gates.yaml"
  [ -f "$model" ] || return 1
  MJ_GATE_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.gates.XXXXXX")"
  mj_yaml_flatten "$model" > "$MJ_GATE_FLAT" 2>/dev/null || {
    rm -f "$MJ_GATE_FLAT"; MJ_GATE_FLAT=""; return 1; }
  [ "$(mj_yget "$MJ_GATE_FLAT" version)" = 1 ] || {
    rm -f "$MJ_GATE_FLAT"; MJ_GATE_FLAT=""; return 1; }
  return 0
}

# every gate the model declares, one per line
mj_gate_ids() { sed -n 's/^gates\.[0-9]*\.id=//p' "$MJ_GATE_FLAT"; }
mj_gate_known() { mj_gate_ids | grep -Fxq "$1"; }
# the index of a gate, or failure
mj_gate_index() {
  awk -F= -v want="$1" '/^gates\.[0-9]+\.id=/ {
      v = $0; sub(/^[^=]*=/, "", v)
      if (v == want) { split($1, k, "."); print k[2]; found = 1; exit } }
    END { exit found ? 0 : 1 }' "$MJ_GATE_FLAT"
}
mj_gate_field() {
  local i; i="$(mj_gate_index "$1")" || return 1
  mj_yget "$MJ_GATE_FLAT" "gates.$i.$2"
}

# mj_gate_inputs <gate> — the pathspecs a run of this gate is evidence over.
#
# Derived from the model, by the model's own rules and no table of its own:
#
#   - a gate the model marks `always: true` is selected by every plan, so any change can
#     invalidate its evidence: its inputs are everything. Saying otherwise would make a
#     run of `shell-lint` survive an edit to a shell script it lints;
#   - otherwise, the union of the paths of every class that names the gate, plus every
#     class whose gates are the word `full` — such a class selects every gate, so its
#     paths belong to every gate's inputs.
#
# This is `Model::inputs_of` in apps/majordomus-cli/src/gates/model.rs, and the two are
# proved equal end to end by case 131: a hash taken here that disagreed with the hash
# recomputed there would make every recorded run read back `stale` at once.
mj_gate_inputs() {
  local gate="$1"
  [ "$(mj_gate_field "$gate" always)" = true ] && { printf '*\n'; return 0; }
  awk -F= -v gate="$gate" '
    /^classes\.[0-9]+\.gates=/       { split($1, k, "."); if ($2 == "full") full[k[2]] = 1 }
    /^classes\.[0-9]+\.gates\.[0-9]+=/ { split($1, k, "."); if ($2 == gate) names[k[2]] = 1 }
    /^classes\.[0-9]+\.paths\.[0-9]+=/ { split($1, k, "."); p[k[2]] = p[k[2]] "\n" $2 }
    END { for (c in p) if ((c in names) || (c in full)) print substr(p[c], 2) }
  ' "$MJ_GATE_FLAT" | sed '/^$/d' | LC_ALL=C sort -u
}

# mj_gate_inputs_hash <gate> — the hash of the tracked files those pathspecs select, in
# git's order, through the one implementation in common.sh.
#
# `set -f` in both subshells, for the reason mj_obligation_inputs_hash documents at length:
# the specs are word-split on purpose and unquoted word-splitting also globs, and a bash
# that expanded `**` before git saw it would be hashing a different set of files than the
# one the pathspec names. Disabling globbing hands git the literal patterns, which it
# matches itself, correctly, at every depth.
mj_gate_inputs_hash() {
  local specs files
  specs="$(mj_gate_inputs "$1" | tr '\n' ' ')"
  [ -n "$specs" ] || { printf ''; return 0; }
  # shellcheck disable=SC2086
  files="$(set -f; cd "$MJ_ROOT" && git ls-files -- $specs 2>/dev/null)"
  [ -n "$files" ] || { printf ''; return 0; }
  # shellcheck disable=SC2086
  ( set -f; cd "$MJ_ROOT" && mj_inputs_hash $files )
}

# ---------------------------------------------------------------- recording a run
# Called from `evidence --gate`. The option shape is `evidence`'s because a gate run is
# evidence: the verb a worker reaches for is the same, and adding a second command for the
# same act would be a second place to look.
mj_gate_record() {
  local gate="$1" exit_code="$2" command="$3" result="$4" json="$5"
  mj_gate_model_load || mj_die "$MJ_EX_MISSING" \
    "this repository declares no CI model at .ai/repo/ci/gates.yaml, so no gate can be recorded"
  mj_gate_known "$gate" || mj_die "$MJ_EX_USAGE" \
    "no gate '$gate' in .ai/repo/ci/gates.yaml; it declares: $(mj_gate_ids | tr '\n' ' ')"
  case "$exit_code" in
    ''|*[!0-9]*) mj_die "$MJ_EX_USAGE" "--exit takes the gate's exit status as a number, not '$exit_code'" ;;
  esac
  mj_load_current || mj_die "$MJ_EX_MISSING" \
    "no active task ($(mj_rel "$MJ_STATE_DIR")/current.yaml); run: majordomus start"
  local task ih extra
  task="$(mj_cur id)"
  ih="$(mj_gate_inputs_hash "$gate")"
  extra="\"task\":\"$task\",\"gate\":\"$gate\",\"exit\":$exit_code,\"inputs_hash\":\"$ih\""
  [ -n "$command" ] && extra="$extra,\"command\":\"$(mj_json_esc "$command")\""
  [ -n "$result" ] && extra="$extra,\"result\":\"$(mj_json_esc "$result")\""
  mj_ledger_append task.gate "$extra"
  if [ "$json" = 1 ]; then
    printf '{"task":"%s","gate":"%s","exit":%s,"inputs_hash":"%s"}\n' "$task" "$gate" "$exit_code" "$ih"
  else
    printf 'evidence: gate %s reported exit %s for %s%s\n' "$gate" "$exit_code" "$task" \
      "$([ -n "$ih" ] && printf ' (inputs %s)' "$(printf '%s' "$ih" | cut -c1-12)")"
    [ "$exit_code" = 0 ] || printf \
      '          note: a failing gate refuses the outcome completed until it reports again\n'
  fi
}

# ---------------------------------------------------------------- the validator
# Dispatched by check and finish through project.completion-is-gated.
#
# It asks the executable and does not judge. `run gates.completion` executes the capability
# in this process — no server, no network — and answers the same document the HTTP route,
# the MCP tool and the Cockpit read, which is the whole point: a worker cannot get a
# different verdict by asking a different surface.
#
# Two things it will not do. It will not refuse over the absence of a verdict: a gate that
# has never reported is `queued`, which is reported and never treated as a pass, but a
# repository where recording is new would otherwise refuse every finish. And it will not
# refuse an outcome that is not `completed`: a task reporting itself blocked is being
# honest, and refusing that teaches a worker to claim completed instead — the rule
# `mj_obl_verdict` already applies to obligations, applied here for the same reason.
mj_gate_verdict() {
  if [ "${MJ_FINISH_OUTCOME:-}" = completed ]; then mj_doctrine_fail gate "$1" "$2" "$3"
  else mj_doctrine_skip gate "$1" "$2 (not refused: the outcome is not completed)"; fi
}

mj_validate_completion_gates() {
  local id bin share out rc=0 finishable blocking unverified g

  if ! mj_load_current; then
    mj_doctrine_skip gate "-" "no active task; no change set to plan gates over"
    MJ_DOCTRINE_SKIPPED=1; return 0
  fi
  id="$(mj_cur id)"

  if ! mj_gate_model_load; then
    mj_doctrine_skip gate "$id" "this repository declares no readable CI model at .ai/repo/ci/gates.yaml; nothing can be planned"
    MJ_DOCTRINE_SKIPPED=1; return 0
  fi

  # shellcheck source=rust_bin.sh
  . "$MJ_LIB_DIR/rust_bin.sh"
  bin="$(mj_rust_bin "$MJ_ROOT")"
  if [ ! -x "$bin" ]; then
    mj_doctrine_skip gate "$id" "the gate reader is not built, so no gate can be judged here (unknown, never a pass)" "bin/majordomus-cli --help"
    MJ_DOCTRINE_SKIPPED=1; return 0
  fi
  if ! command -v jq >/dev/null 2>&1; then
    mj_doctrine_skip gate "$id" "jq is not installed, so the judgement cannot be read here (unknown, never a pass)"
    MJ_DOCTRINE_SKIPPED=1; return 0
  fi

  # the repository's own distribution when it has one, else the tool's, which is where
  # mj_obligations_load already reads the shipped vocabulary from. A managed repository is
  # not a distribution and carries no share/ of its own.
  share="$(mj_rust_share "$MJ_ROOT")"
  [ -n "$share" ] || { [ -f "$MJ_BIN_DIR/../share/kinds.yaml" ] && share="$MJ_BIN_DIR/../share"; }
  # `run` answers with the execution envelope — the id, the state, the timings — and the
  # capability's own document under `.output`. That envelope is what makes an execution
  # inspectable afterwards, so it is unwrapped here rather than asked for without it.
  out="$( ( [ -z "$share" ] || export MAJORDOMUS_SHARE="$share"
            "$bin" run gates.completion --input '{}' --quiet --format json --repo "$MJ_ROOT" ) 2>/dev/null \
          | jq -c '.output // empty' 2>/dev/null )" || rc=$?
  if [ "$rc" != 0 ] || [ -z "$out" ]; then
    mj_doctrine_skip gate "$id" "the executable could not answer gates.completion (exit $rc), so no gate can be judged here (unknown, never a pass)" "$bin run gates.completion --input '{}' --format json"
    MJ_DOCTRINE_SKIPPED=1; return 0
  fi

  # `has`, not `//`: jq's alternative operator treats `false` as absent, so `.finishable //
  # empty` reads a refusal as an unreadable answer — which would turn every refusal into a
  # skip, silently, in the one place that must not be generous
  finishable="$(printf '%s' "$out" | jq -r 'if has("finishable") then (.finishable | tostring) else empty end' 2>/dev/null)"
  if [ -z "$finishable" ]; then
    mj_doctrine_skip gate "$id" "gates.completion answered something this validator cannot read (unknown, never a pass)"
    MJ_DOCTRINE_SKIPPED=1; return 0
  fi
  blocking="$(printf '%s' "$out" | jq -r '(.blocking // [])[]' 2>/dev/null)"
  unverified="$(printf '%s' "$out" | jq -r '(.unverified // [])[]' 2>/dev/null)"

  # where the task stands, in one line: the stage the policy's fold derived, and what it
  # still owes there. Derived, never recorded — share/completion.yaml is the one definition
  # of done and this is its projection for a terminal.
  local stage stage_state stage_owing complete
  stage="$(printf '%s' "$out" | jq -r '.stage.title // empty' 2>/dev/null)"
  stage_state="$(printf '%s' "$out" | jq -r '.stage.state // empty' 2>/dev/null)"
  stage_owing="$(printf '%s' "$out" | jq -r '(.stage.owing // []) | join(" ")' 2>/dev/null)"
  complete="$(printf '%s' "$out" | jq -r 'if has("complete") then (.complete | tostring) else "false" end' 2>/dev/null)"
  [ -z "$stage" ] || mj_info stage "$id" "$stage — $stage_state${stage_owing:+ (owing: $stage_owing)}" \
    "$bin run gates.completion --input '{}' --format json | jq '.output.stage'"
  # for the finish record: where the task stood when the outcome was taken, so that a task
  # closed short of completed carries the stage it stopped at and not only the word
  MJ_COMPLETION_STAGE="$(printf '%s' "$out" | jq -r '.stage.id // empty' 2>/dev/null)"
  MJ_COMPLETION_COMPLETE="$complete"
  export MJ_COMPLETION_STAGE MJ_COMPLETION_COMPLETE

  # what is known to be wrong, one finding per gate, each carrying the gate's own reason
  # and the command that would settle it
  for g in $blocking; do
    mj_gate_verdict "$g" \
      "$(printf '%s' "$out" | jq -r --arg g "$g" '.gates[] | select(.id == $g) | "\(.status): \(.reason)"' 2>/dev/null)" \
      "$(printf '%s' "$out" | jq -r --arg g "$g" '.gates[] | select(.id == $g) | .remediation' 2>/dev/null)"
  done

  # what never reported. Never a pass and never a refusal: a debt, named, with the rule
  # that says why silence is not green.
  if [ -n "$unverified" ]; then
    mj_doctrine_skip gate "$id" \
      "$(printf '%s' "$unverified" | wc -l | tr -d ' ') required gate(s) have never reported: $(printf '%s' "$unverified" | tr '\n' ' ')— never-reported is not green (project.never-reported-is-not-green)" \
      "majordomus evidence --gate <id> --exit <status>"
  fi

  # what the change set implies and the task never promised: reported, never refused here.
  # A declared obligation is `mj_validate_obligations`' to judge; this names the gap between
  # what the paths oblige and what the task said it owes, which nothing else reports.
  local ob
  for ob in $(printf '%s' "$out" | jq -r '(.obligations // [])[] | select(.applicable and (.declared | not)) | .id' 2>/dev/null); do
    mj_doctrine_skip gate "obligation:$ob" \
      "the change implies '$ob' and the task does not declare it: $(printf '%s' "$out" | jq -r --arg o "$ob" '.obligations[] | select(.id == $o) | .reason' 2>/dev/null)" \
      "majordomus start --requires $ob"
  done

  # the done invariant, in one line rather than nineteen: what a reader must still settle,
  # each question with the source that would answer it. `check` is read routinely and a
  # nineteen-line block would be scrolled past; the whole set is in the JSON.
  local owing
  owing="$(printf '%s' "$out" | jq -r '[(.questions // [])[] | select(.status != "pass" and .status != "exempt") | "\(.id)=\(.status)"] | join(" ")' 2>/dev/null)"
  [ -z "$owing" ] || mj_doctrine_skip "done" "$id" "the done invariant is not yet answered: $owing" \
    "$bin run gates.completion --input '{}' --format json | jq '.output.questions'"

  # `completed` is earned. When the policy says so, the outcome completed is refused unless
  # the report's one derived bit — `complete`: every question of share/completion.yaml
  # passes or is exempt — is true, and the refusal names each question still owed with the
  # command that settles it. Nothing is re-judged here: the bit is the executable's, the same
  # one the HTTP route, the MCP tool and the Cockpit show (rule project.completion-is-proved).
  if [ "${MJ_FINISH_OUTCOME:-}" = completed ] && [ "$(mj_pol verification.completed_means_complete)" = true ] && [ "$complete" != true ]; then
    local q
    while IFS=$'\t' read -r q qstatus qrem; do
      [ -n "$q" ] || continue
      mj_doctrine_fail "done" "$q" "$qstatus: $(printf '%s' "$out" | jq -r --arg q "$q" '.questions[] | select(.id == $q) | .evidence' 2>/dev/null)" "$qrem"
    done <<EOF
$(printf '%s' "$out" | jq -r '(.questions // [])[] | select(.status != "pass" and .status != "exempt") | "\(.id)\t\(.status)\t\(.remediation)"' 2>/dev/null)
EOF
    return 0
  fi

  if [ -z "$blocking" ] && [ -z "$unverified" ]; then
    mj_doctrine_ok gate "$id" "every gate the change selects has reported over this tree ($(printf '%s' "$out" | jq -r '(.tallies.pass // 0)') passing, $(printf '%s' "$out" | jq -r '(.tallies.exempt // 0)') not applicable)"
  fi
  return 0
}

#!/usr/bin/env bash
# shellcheck disable=SC2034  # the MJ_BIND_* outputs are read by the commands that ask
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_intent_binding:-}" ] && return 0 || MJ_LIB_intent_binding=1
# intent_binding — what a task is bound to, asked of the built executable (ADR 0111).
#
# The task lifecycle is this shell; the intent engine is the Rust executable. Which intent a
# piece of work serves is decided in one place, `intents.binding`, and everything here asks
# it and decides nothing: `start` asks before it writes a task record, `context` asks to
# brief the worker, `handover` asks for the pins it writes, `handover --resolve` asks again
# to compare them, and `plan start` asks before it stamps.
#
# An answer that cannot be read is *unknown*, and unknown is never a pass: each caller is
# told why (MJ_BIND_WHY) and what would settle it (MJ_BIND_FIX), and decides under the
# policy what unknown costs it. That is the rule the completion-gates validator already
# follows for the same reason — a gate that reads silence as consent is not a gate.

# mj_ledger_append-writes: opposition.recorded
#
# The line above is read by test/cases/33_event_registry.sh, which holds every declared event
# to something that writes it. No shell writes this one: `majordomus-cli intent stamp` appends
# it from the Rust executable (capability intent_opposition.record, ADR 0112), and this file
# is the shell's side of that executable's intent engine, so the declaration lives here.

# mj_binding_mode — `off`, `advisory` or `required`: the policy's `intent.binding`, with
# anything else (absent, misspelt) read as `off`, which is what a policy that predates the
# key means. doctor reports a misspelt value; start must not guess a stricter one.
mj_binding_mode() {
  case "$(mj_pol intent.binding)" in
    advisory) printf 'advisory' ;;
    required) printf 'required' ;;
    *)        printf 'off' ;;
  esac
}

# mj_binding_ask <issue> <intent> <paths,comma> <exemption> <because>
# Asks intents.binding. On an answer that was read: returns 0 with MJ_BIND_JSON holding the
# capability's document and MJ_BIND_STANDING its standing. On no readable answer: returns 1
# with MJ_BIND_WHY and MJ_BIND_FIX set, and MJ_BIND_JSON empty.
mj_binding_ask() {
  local issue="$1" intent="$2" paths="$3" exemption="$4" because="$5"
  local bin share input out rc=0
  MJ_BIND_JSON=""; MJ_BIND_STANDING=""; MJ_BIND_WHY=""; MJ_BIND_FIX=""

  if ! command -v jq >/dev/null 2>&1; then
    MJ_BIND_WHY="jq is not installed, so the binding cannot be read here"
    MJ_BIND_FIX="brew install jq"
    return 1
  fi
  # shellcheck source=rust_bin.sh
  . "$MJ_LIB_DIR/rust_bin.sh"
  bin="$(mj_rust_bin "$MJ_HOME")"
  if [ ! -x "$bin" ]; then
    MJ_BIND_WHY="the intent engine is not built at $bin, so the binding cannot be asked here"
    MJ_BIND_FIX="bin/majordomus-cli --help"
    return 1
  fi
  input="$(jq -cn --arg issue "$issue" --arg intent "$intent" --arg paths "$paths" \
              --arg exemption "$exemption" --arg because "$because" \
    '{paths: $paths}
     + (if $issue     != "" then {issue: $issue}         else {} end)
     + (if $intent    != "" then {intent: $intent}       else {} end)
     + (if $exemption != "" then {exemption: $exemption} else {} end)
     + (if $because   != "" then {because: $because}     else {} end)')" || {
    MJ_BIND_WHY="the binding request could not be written as JSON"
    MJ_BIND_FIX="jq --version"
    return 1
  }

  # the repository's own distribution when it has one, else the tool's (as the gates do)
  share="$(mj_rust_share "$MJ_ROOT")"
  [ -n "$share" ] || { [ -f "$MJ_BIN_DIR/../share/kinds.yaml" ] && share="$MJ_BIN_DIR/../share"; }
  # `run` answers with the execution envelope and the capability's document under
  # `.output`; the executable's own exit is kept apart from jq's
  out="$( [ -z "$share" ] || export MAJORDOMUS_SHARE="$share"
          "$bin" run intents.binding --input "$input" --quiet --format json --repo "$MJ_ROOT" 2>/dev/null )" || rc=$?
  out="$(printf '%s' "$out" | jq -c '.output // empty' 2>/dev/null)" || out=""
  MJ_BIND_STANDING="$(printf '%s' "$out" | jq -r '.standing // empty' 2>/dev/null)"
  if [ "$rc" != 0 ] || [ -z "$out" ] || [ -z "$MJ_BIND_STANDING" ]; then
    MJ_BIND_STANDING=""
    MJ_BIND_WHY="the executable could not answer intents.binding (exit $rc)"
    MJ_BIND_FIX="$bin run intents.binding --input '$input' --format json"
    return 1
  fi
  MJ_BIND_JSON="$out"
  return 0
}

# mj_binding_get <jq filter> — one value of the last answer, empty when there is none.
mj_binding_get() {
  [ -n "${MJ_BIND_JSON:-}" ] || return 0
  printf '%s' "$MJ_BIND_JSON" | jq -r "$1" 2>/dev/null
}

# mj_binding_ask_task — the binding of the active task as its record names it: the issue or
# the intent it named, with its scope as the paths; an exemption with its reason; or, when
# it named nothing, the scope alone. Requires mj_load_current to have succeeded.
mj_binding_ask_task() {
  local scope; scope="$(mj_ylist "$MJ_CUR_FLAT" scope | paste -sd, -)"
  if [ -n "$(mj_cur exemption)" ]; then
    mj_binding_ask "" "" "" "$(mj_cur exemption)" "$(mj_cur exemption_because)"
  else
    mj_binding_ask "$(mj_cur issue)" "$(mj_cur intent)" "$scope" "" ""
  fi
}

# mj_task_names_work — whether the active task's record says what it serves at all.
mj_task_names_work() {
  [ -n "$(mj_cur issue)$(mj_cur intent)$(mj_cur exemption)" ]
}

# mj_binding_section — the INTENT section of `majordomus context` for the active task, from
# the last answer. Bounded: <max> lines at most, and what is cut first is what a worker can
# fetch again with the command the last line names — gap conditions, then non-goals.
mj_binding_section() {
  local max="${1:-40}"
  printf '%s' "$MJ_BIND_JSON" | jq -r --argjson max "$max" '
    def crit: "criterion    \(.id)  \(.state)  — \(.criterion)";
    def lines(withgap):
      [ "standing     \(.standing)" ]
      + (if .exemption then ["exempt       \(.exemption.class) — \(.exemption.because)"] else [] end)
      + [ .issues[]? | "issue        \(.issue)  \(.verdict)  milestone \(.milestone)" ]
      + [ .intents[]? |
          ( "intent       \(.id)  \(.stage)  — \(.title)",
            "statement    \(.statement)",
            (.criteria[]? | crit),
            (.invariants[]? | "invariant    \(.)"),
            (.guards[]? | "guard        \(.id)  \(if .violated then "violated" elif .state == "current" then "holds" else "not judged" end)  \(.state)  — \(.invariant)"),
            (if .critique then
               "critique     reviewed at \(.critique.reviewed_at); open blocking: \(
                 if (.critique.open_blocking | length) == 0 then "none"
                 else ([.critique.open_blocking[].id] | join(", ")) end)"
             else "critique     none recorded" end),
            (if withgap and .gap then
               (.gap.conditions[]? | "gap          \(.criterion) \(.state_text // .state)")
             else empty end) ) ]
      + [ .reviews[]? | "review       \(.intent)  \(.state)  \(.disposition)\(
            if (.reviewed_revision // "") != "" then "  stamped \(.reviewed_revision[0:12])" else "" end)" ]
      + [ .notes[]? | "note         \(.)" ]
      + [ .refusals[]? | "refusal      \(.cause)  \(.message)" ]
      + (if (.plan_revision // "") != "" then
           ["pins         plan \(.plan_revision[0:12])  evidence \(.evidence_standing[0:12])"]
         else [] end);
    (lines(true)) as $full
    | (if ($full | length) <= $max then $full else lines(false) end) as $fit
    | (if ($fit | length) <= $max then $fit
       else $fit[0:($max - 1)] + ["...          \(($fit | length) - $max + 1) more line(s): majordomus-cli intent binding"] end)
    | .[]' 2>/dev/null
}

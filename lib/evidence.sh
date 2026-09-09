#!/usr/bin/env bash
# evidence — what a task owes before it may be called completed, and the evidence that
# discharges each one.
#
# A task's scope says where a worker may write; its `requires` says what the worker owes.
# The two are different promises: a change can sit entirely inside its scope and still be
# uncommitted on a laptop. The vocabulary is data in share/obligations.yaml, so a token is
# added there and not here, and doctor proves in both directions that every token names a
# reachable check and that no check hides under no token.
#
# Evidence is a ledger line, not a file. The ledger is already append-only, ordered and
# integrity-checked, and its envelope already carries the head, the branch and the session.
# What the line adds is the hash of the inputs the evidence was taken over, which is what
# lets a later reader say the evidence no longer describes this tree — stale rather than
# merely old. The vocabulary for that judgement is mj_git_label's, not a new one.

MJ_OBL_FLAT=""
mj_obligations_load() {
  [ -n "$MJ_OBL_FLAT" ] && [ -f "$MJ_OBL_FLAT" ] && return 0
  local reg="$MJ_BIN_DIR/../share/obligations.yaml"
  [ -f "$reg" ] || mj_die "$MJ_EX_INTERNAL" "obligation vocabulary missing: $reg"
  MJ_OBL_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.ob.XXXXXX")"
  mj_yaml_flatten "$reg" > "$MJ_OBL_FLAT" 2>/dev/null \
    || mj_die "$MJ_EX_INTERNAL" "obligation vocabulary does not parse: $reg"
  [ "$(mj_yget "$MJ_OBL_FLAT" version)" = 1 ] || mj_die "$MJ_EX_INTERNAL" "obligation vocabulary version must be 1"
}

# These read the loaded vocabulary; mj_obligations_load must run in the calling shell
# first, for the reason mj_events_load documents.
mj_obligation_ids() { sed -n 's/^obligations\.[0-9]*\.id=//p' "$MJ_OBL_FLAT"; }
mj_obligation_known() { mj_obligation_ids | grep -Fxq "$1"; }
mj_obligation_index() {
  awk -F= -v want="$1" '/^obligations\.[0-9]+\.id=/ { v = $0; sub(/^[^=]*=/, "", v); if (v == want) { split($1, k, "."); print k[2]; found = 1; exit } }
    END { exit found ? 0 : 1 }' "$MJ_OBL_FLAT"
}
mj_obligation_field() {
  local i; i="$(mj_obligation_index "$1")" || return 1
  mj_yget "$MJ_OBL_FLAT" "obligations.$i.$2"
}
# the pathspecs an obligation's evidence is hashed over; empty for a remote fact, which is
# bound to a commit instead of to a tree
mj_obligation_inputs() {
  local i; i="$(mj_obligation_index "$1")" || return 1
  sed -n "s/^obligations\.$i\.inputs\.[0-9]*=//p" "$MJ_OBL_FLAT"
}
mj_obligation_remote() { [ "$(mj_obligation_field "$1" remote)" = "true" ]; }

# The hash the evidence is taken over: the tracked files the obligation's pathspecs select,
# in git's order, through the one implementation in common.sh. An obligation with no inputs
# hashes to the empty string and is judged by its commit alone.
mj_obligation_inputs_hash() {
  local tok="$1" specs files
  specs="$(mj_obligation_inputs "$tok")"
  [ -n "$specs" ] || { printf ''; return 0; }
  # shellcheck disable=SC2086
  files="$(cd "$MJ_ROOT" && git ls-files -- $specs 2>/dev/null)"
  [ -n "$files" ] || { printf ''; return 0; }
  # shellcheck disable=SC2086
  ( cd "$MJ_ROOT" && mj_inputs_hash $files )
}

# ---------------------------------------------------------------- the task's obligations
# What the active task declared. Absent means it owes nothing beyond the rest of the
# contract, which is how every task behaved before obligations existed.
# Reads the record mj_load_current already flattened, so the caller loads once and every
# reader here sees the same bytes.
mj_task_requires() {
  [ -n "${MJ_CUR_FLAT:-}" ] || return 0
  sed -n 's/^requires\.[0-9]*=//p' "$MJ_CUR_FLAT"
}

# The most recent evidence line for a token of the active task, as
# "<head>\t<inputs_hash>\t<ts>". Nothing older is consulted: evidence is superseded by
# evidence, and the ledger keeps the history for a reader who wants it.
mj_obligation_evidence() {
  local task="$1" tok="$2"
  local ledger="$MJ_STATE_DIR/ledger.jsonl"
  [ -f "$ledger" ] || return 1
  awk -v task="$task" -v tok="$tok" '
    index($0, "\"event\":\"task.evidence\"") == 0 { next }
    index($0, "\"task\":\"" task "\"") == 0 { next }
    index($0, "\"covers\":\"" tok "\"") == 0 { next }
    { line = $0 }
    END {
      if (line == "") exit 1
      h = line; sub(/.*"head":"/, "", h); sub(/".*/, "", h)
      x = line; sub(/.*"inputs_hash":"/, "", x); sub(/".*/, "", x)
      t = line; sub(/.*"ts":"/, "", t); sub(/".*/, "", t)
      printf "%s\t%s\t%s\n", h, x, t
    }' "$ledger"
}

# ---------------------------------------------------------------- recording evidence
# majordomus evidence --covers <token> [--type <t>] --command <cmd> | --artifact <ref>
#
# The option shape is plan's, so a reader learns it once. What differs is where it lands:
# an issue's evidence is written into the issue, because the contract and its proof belong
# in one record; a task's evidence is a ledger line, because a task's record is the local
# state of one checkout and the ledger is what already survives it.
mj_evidence() {
  local covers="" etype="manual" ecmd="" eart="" eres="" json=0 task ih
  # a bare `evidence` is a question, not a mistake: it prints what it needs, as every
  # other command here does, and exits 2
  [ $# -gt 0 ] || { mj_evidence_usage >&2; return "$MJ_EX_USAGE"; }
  while [ $# -gt 0 ]; do
    case "$1" in
      --covers) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--covers needs a token"; covers="$2"; shift 2 ;;
      --covers=*) covers="${1#--covers=}"; shift ;;
      --type) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--type needs a value"; etype="$2"; shift 2 ;;
      --type=*) etype="${1#--type=}"; shift ;;
      --command) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--command needs a command"; ecmd="$2"; shift 2 ;;
      --command=*) ecmd="${1#--command=}"; shift ;;
      --artifact) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--artifact needs a reference"; eart="$2"; shift 2 ;;
      --artifact=*) eart="${1#--artifact=}"; shift ;;
      --result) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--result needs a value"; eres="$2"; shift 2 ;;
      --result=*) eres="${1#--result=}"; shift ;;
      --json) json=1; shift ;;
      -h|--help) mj_evidence_usage; return 0 ;;
      *) mj_die "$MJ_EX_USAGE" "evidence: unknown option $1" ;;
    esac
  done
  mj_require_installed
  mj_obligations_load
  [ -n "$covers" ] || mj_die "$MJ_EX_USAGE" "evidence needs --covers <token>; one of: $(mj_obligation_ids | tr '\n' ' ')"
  mj_obligation_known "$covers" || mj_die "$MJ_EX_USAGE" \
    "no obligation '$covers'; share/obligations.yaml declares: $(mj_obligation_ids | tr '\n' ' ')"
  [ -n "$ecmd" ] || [ -n "$eart" ] || mj_die "$MJ_EX_USAGE" "evidence needs --command or --artifact; narrative is not evidence"
  mj_load_current || mj_die "$MJ_EX_MISSING" "no active task ($(mj_rel "$MJ_STATE_DIR")/current.yaml); run: majordomus start"
  task="$(mj_cur id)"
  # The task must have declared the obligation. Recording evidence for something nobody
  # promised is how a checklist grows entries nobody asked for.
  mj_task_requires | grep -qx -- "$covers" || mj_die "$MJ_EX_REFUSED" \
    "the active task does not require '$covers' (it requires: $(mj_task_requires | paste -sd, -))"
  ih="$(mj_obligation_inputs_hash "$covers")"
  local extra="\"task\":\"$task\",\"covers\":\"$covers\",\"kind\":\"$etype\",\"inputs_hash\":\"$ih\""
  [ -n "$ecmd" ] && extra="$extra,\"command\":\"$(mj_json_esc "$ecmd")\""
  [ -n "$eart" ] && extra="$extra,\"artifact\":\"$(mj_json_esc "$eart")\""
  [ -n "$eres" ] && extra="$extra,\"result\":\"$(mj_json_esc "$eres")\""
  mj_ledger_append task.evidence "$extra"
  if [ "$json" = 1 ]; then
    printf '{"task":"%s","covers":"%s","kind":"%s","inputs_hash":"%s"}\n' "$task" "$covers" "$etype" "$ih"
  else
    printf 'evidence: %s recorded for %s%s\n' "$covers" "$task" \
      "$([ -n "$ih" ] && printf ' (inputs %s)' "$(printf '%s' "$ih" | cut -c1-12)")"
  fi
}

mj_evidence_usage() {
  cat <<USAGE
usage: majordomus evidence --covers <token> [--type <kind>] (--command <cmd> | --artifact <ref>) [--result <r>] [--json]

  Record that one obligation the active task declared has been discharged. The evidence is
  a ledger line carrying the hash of the files the obligation names, so that changing any
  of them makes the evidence stale rather than merely old.

  --covers    the obligation; share/obligations.yaml declares them
  --type      how it was taken (test, build, ci, artifact, manual); default manual
  --command   the command that produced it — narrative is not evidence
  --artifact  a reference the evidence points at, such as a published URL
  --result    what it said, when a command's output is the point

  exit 0 recorded, 2 on usage, 11 when no task is active or the task did not promise it
USAGE
}

# ---------------------------------------------------------------- doctrine validator
# Dispatched by check and finish through majordomus.obligation-closure. Skipped for every
# outcome but completed, as the verification and profile lines already are: a task that
# reports itself blocked is being honest, and refusing it would teach a worker to lie.
# One place decides whether a shortfall refuses or merely reports: the outcome does.
# mj_obl_verdict <subject> <message> <reproduce>
mj_obl_verdict() {
  if [ "${MJ_FINISH_OUTCOME:-}" = completed ]; then mj_doctrine_fail obligation "$1" "$2" "$3"
  else mj_doctrine_skip obligation "$1" "$2 (not refused: the outcome is not completed)"; fi
}

mj_validate_obligations() {
  local toks tok ev head_rec ih_rec ih_now label rel
  mj_load_current || { mj_doctrine_skip obligation "-" "no active task; nothing owes anything"; return 0; }
  toks="$(mj_task_requires)"
  if [ -z "$toks" ]; then
    mj_doctrine_skip obligation "$(mj_cur id)" "the task declares no obligations"
    return 0
  fi
  # Every obligation is judged on every run, so that `check` can say an evidence has gone
  # stale before a worker builds an afternoon on it. Only work that claims to be done is
  # refused: an outcome that is not completed is a worker being honest, and refusing it
  # would teach the worker to claim completed instead. `mj_obl_verdict` carries that
  # distinction so the judgement itself is written once.
  mj_obligations_load
  rel="$(mj_rel "$MJ_STATE_DIR")/current.yaml"
  for tok in $toks; do
    if ! mj_obligation_known "$tok"; then
      mj_obl_verdict "$rel" "the task requires '$tok', which share/obligations.yaml does not declare" "majordomus evidence --help"
      continue
    fi
    if ! ev="$(mj_obligation_evidence "$(mj_cur id)" "$tok")"; then
      mj_obl_verdict "$tok" "owed, and no evidence was recorded" \
        "majordomus evidence --covers $tok --command '$(mj_obligation_field "$tok" discharged_by)'"
      continue
    fi
    head_rec="$(printf '%s' "$ev" | cut -f1)"
    ih_rec="$(printf '%s' "$ev" | cut -f2)"
    # A remote fact is bound to the commit it was taken about, not to the tree: the site
    # that serves a commit goes on serving it while the tree moves underneath.
    if mj_obligation_remote "$tok"; then
      label="$(mj_git_label "$head_rec" "$(mj_git_branch)")"
      case "$label" in
        exact) mj_doctrine_ok obligation "$tok" "discharged at this commit" ;;
        advanced) mj_obl_verdict "$tok" "the evidence names $(printf '%s' "$head_rec" | cut -c1-12), and the branch has moved since; the fact it proved is about the older commit" "majordomus evidence --covers $tok --command '$(mj_obligation_field "$tok" discharged_by)'" ;;
        *) mj_obl_verdict "$tok" "the evidence was taken in a $label context ($(printf '%s' "$head_rec" | cut -c1-12)); it does not describe this branch" "majordomus evidence --covers $tok --command '$(mj_obligation_field "$tok" discharged_by)'" ;;
      esac
      continue
    fi
    ih_now="$(mj_obligation_inputs_hash "$tok")"
    if [ "$ih_rec" = "$ih_now" ]; then
      mj_doctrine_ok obligation "$tok" "discharged over inputs $(printf '%s' "$ih_now" | cut -c1-12)"
    else
      mj_obl_verdict "$tok" \
        "the evidence was taken over inputs $(printf '%s' "$ih_rec" | cut -c1-12) and this tree hashes to $(printf '%s' "$ih_now" | cut -c1-12); it no longer describes what it proved" \
        "majordomus evidence --covers $tok --command '$(mj_obligation_field "$tok" discharged_by)'"
    fi
  done
  return 0
}

# The dispatcher's entry point. `evidence` is the verb because recording is what a worker
# does here; the obligations themselves are read through `check`, which already reports
# what is outstanding, rather than through a second listing command nobody would run.
mj_cmd_evidence() { mj_evidence "$@"; }

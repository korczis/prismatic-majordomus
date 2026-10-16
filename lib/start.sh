#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_start:-}" ] && return 0 || MJ_LIB_start=1
# start — begin a scoped task under a profile. One active task per checkout.
# shellcheck source=handover.sh
. "$MJ_LIB_DIR/handover.sh"
# shellcheck source=check.sh
. "$MJ_LIB_DIR/check.sh"
# shellcheck source=intent_binding.sh
. "$MJ_LIB_DIR/intent_binding.sh"
mj_cmd_start() {
  local task="" scope="" requires="" profile="" owner="${USER:-unknown}"
  local issue="" intent="" exempt="" because=""
  while [ $# -gt 0 ]; do case "$1" in
    --scope) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--scope needs paths"; scope="$scope,$2"; shift 2 ;;
    --scope=*) scope="$scope,${1#--scope=}"; shift ;;
    --requires) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--requires needs obligation tokens"; requires="$requires,$2"; shift 2 ;;
    --requires=*) requires="$requires,${1#--requires=}"; shift ;;
    --profile) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--profile needs a name"; profile="$2"; shift 2 ;;
    --profile=*) profile="${1#--profile=}"; shift ;;
    --owner) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--owner needs a value"; owner="$2"; shift 2 ;;
    --issue) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--issue needs an issue id"; issue="$2"; shift 2 ;;
    --issue=*) issue="${1#--issue=}"; shift ;;
    --intent) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--intent needs an intent id"; intent="$2"; shift 2 ;;
    --intent=*) intent="${1#--intent=}"; shift ;;
    --exempt) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--exempt needs an exemption class"; exempt="$2"; shift 2 ;;
    --exempt=*) exempt="${1#--exempt=}"; shift ;;
    --because) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--because needs a reason"; because="$2"; shift 2 ;;
    --because=*) because="${1#--because=}"; shift ;;
    --help|-h) cat <<H
usage: majordomus start "<task>" --scope <path>[,<path>...] [--requires <token>[,<token>...]]
                       [--profile <name>] [--owner <who>]
                       [--issue <id> | --intent <id> | --exempt <class> --because "<reason>"]
  one active task per checkout; refuses (15) while a task is active — handover or finish it first
  scope paths are normalised (no trailing /, no escapes); overlap with other worktrees is reported
  --requires  what this task owes before it may be called completed: tokens of
              share/obligations.yaml, listed by \`majordomus evidence\` with no arguments.
              A scope says where a worker may write; requires says what the worker owes.
              \`check\` reports each one, \`evidence --covers\` discharges the ones a worker
              records, and \`finish --outcome completed\` refuses while any is outstanding.
  --issue     the issue this task executes; --intent the intent it serves, named directly;
              --exempt a class the policy declares under intent.exemptions, with --because.
              What the task is bound to is asked of \`majordomus-cli intent binding\` once, here:
              under policy intent.binding: required a refused binding does not start (10),
              under advisory it is reported, and with the key absent it is asked only when
              one of these flags is given. The record keeps what was named and two pins a
              handover carries; the intent itself is derived on every read (ADR 0111).
H
      return 0 ;;
    -*) mj_die "$MJ_EX_USAGE" "start: unknown option $1" ;;
    *) [ -z "$task" ] || mj_die "$MJ_EX_USAGE" "start: task must be one argument (quote it)"; task="$1"; shift ;;
  esac; done
  [ -n "$task" ]  || mj_die "$MJ_EX_USAGE" "start: a task description is required"
  scope="${scope#,}"; requires="${requires#,}"
  [ -n "$(printf '%s' "$scope" | tr -d ', ')" ] || mj_die "$MJ_EX_USAGE" "start: --scope is required (comma-separated repository paths)"
  mj_require_installed
  mj_load_policy || mj_die "$MJ_EX_CONTRACT" "policy does not parse (run: majordomus doctor)"
  [ -n "$profile" ] || profile="$(mj_pol profiles.default)"
  mj_load_profile "$profile" || mj_die "$MJ_EX_MISSING" "no profile '$profile' ($(mj_rel "$MJ_PROFILES_DIR")/$profile.yaml)"

  # existing task? One active task per checkout — so a record belonging to another checkout
  # does not block this one. It is replaced in this working copy and left alone everywhere
  # else; local state is never tracked, so nothing about the replacement travels.
  if mj_load_current; then
    local oc; oc="$(mj_cur outcome)"
    if mj_task_is_foreign; then
      mj_warn task "$(mj_cur id)" "the record here belongs to $(mj_cur worktree); replacing it in this working copy only" "cat $(mj_rel "$MJ_STATE_DIR")/current.yaml"
      [ "$oc" = active ] && mj_info task "$(mj_cur id)" "that task is still active there; nothing here changes it" "majordomus --repo $(mj_cur worktree) check"
    else
      case "$oc" in
        active) mj_die "$MJ_EX_REFUSED" "task $(mj_cur id) is active ('$(mj_cur task)'); run majordomus handover or majordomus finish first" ;;
        *) mkdir -p "$MJ_STATE_DIR/archive"; mv "$MJ_CUR" "$MJ_STATE_DIR/archive/$(mj_cur id).yaml" ;;
      esac
    fi
  fi

  # normalise scope
  local raw p norm="" IFS_save="$IFS"; IFS=','
  for raw in $scope; do
    IFS="$IFS_save"
    raw="$(printf '%s' "$raw" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//')"
    [ -n "$raw" ] || { IFS=','; continue; }
    p="$(mj_norm_path "$raw")" || mj_die "$MJ_EX_USAGE" "start: scope path '$raw' is empty, absolute, or escapes the repository"
    norm="$norm $p"; IFS=','
  done; IFS="$IFS_save"

  # The obligations this task owes. Validated here against the shipped vocabulary rather
  # than at `evidence` time, because a token misspelt at `start` would otherwise sit in the
  # record until the moment a worker tried to discharge it and was told the task never
  # asked for it — the failure would arrive at the end of the work instead of the beginning.
  local req_norm="" tok
  if [ -n "$(printf '%s' "$requires" | tr -d ', ')" ]; then
    mj_obligations_load
    IFS=','; for tok in $requires; do
      IFS="$IFS_save"
      tok="$(printf '%s' "$tok" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//')"
      [ -n "$tok" ] || { IFS=','; continue; }
      mj_obligation_known "$tok" || mj_die "$MJ_EX_USAGE" "start: '$tok' is not an obligation ($(mj_obligation_ids | paste -sd, -))"
      req_norm="$req_norm $tok"; IFS=','
    done; IFS="$IFS_save"
  fi

  # What the task is bound to, asked once, before anything is written. A flag is a question
  # the worker put, so it is asked under every policy and an answer that cannot be read
  # refuses; with no flag the policy decides whether the scope alone is asked at all.
  local bind_mode bind_asked=0 bind_named=0 bind_standing="" plan_rev="" ev_standing=""
  bind_mode="$(mj_binding_mode)"
  [ -n "$issue$intent$exempt" ] && bind_named=1
  [ -z "$because" ] || [ -n "$exempt" ] || mj_die "$MJ_EX_USAGE" "start: --because gives the reason of --exempt; it was given alone"
  if [ "$bind_named" = 1 ] || [ "$bind_mode" != off ]; then
    bind_asked=1
    if mj_binding_ask "$issue" "$intent" "$(printf '%s' "$norm" | sed 's/^ //; s/ /,/g')" "$exempt" "$because"; then
      bind_standing="$MJ_BIND_STANDING"
      plan_rev="$(mj_binding_get '.plan_revision // empty')"
      ev_standing="$(mj_binding_get '.evidence_standing // empty')"
      if [ "$bind_standing" = refused ]; then
        if [ "$bind_named" = 1 ] || [ "$bind_mode" = required ]; then
          mj_binding_get '.refusals[] | "refused  \(.cause)  \(.message)"' >&2
          mj_die "$MJ_EX_REFUSED" "the task is bound to nothing it may start under; name the issue it executes (--issue), the intent it serves (--intent), or an exemption the policy declares (--exempt <class> --because \"<reason>\"); see: majordomus-cli intent binding --help"
        fi
        mj_warn intent "-" "this task's binding is refused ($(mj_binding_get '[.refusals[].cause] | join(", ")')); policy intent.binding is advisory, so it starts" "majordomus-cli intent binding --path $(printf '%s' "$norm" | sed 's/^ //; s/ / --path /g')"
      fi
    elif [ "$bind_named" = 1 ] || [ "$bind_mode" = required ]; then
      mj_die "$MJ_EX_REFUSED" "$MJ_BIND_WHY; the binding is unknown, and unknown is never a pass (run: $MJ_BIND_FIX)"
    else
      mj_warn intent "-" "$MJ_BIND_WHY; the binding is unknown and policy intent.binding is advisory, so the task starts unbound" "$MJ_BIND_FIX"
    fi
  fi

  local id; id="t-$(mj_now_compact | tr -d 'TZ')-$(mj_rand16 | cut -c1-4)"
  local now; now="$(mj_now)"
  mkdir -p "$MJ_STATE_DIR"
  {
    printf 'id: %s\ntask: "%s"\nprofile: %s\nowner: "%s"\nscope:\n' "$id" "$(printf '%s' "$task" | sed 's/"/\\"/g')" "$profile" "$owner"
    for p in $norm; do printf '  - %s\n' "$p"; done
    if [ -n "$req_norm" ]; then printf 'requires:\n'; for tok in $req_norm; do printf '  - %s\n' "$tok"; done; fi
    # what the worker named, and two pins; never the intent's stage, verdict or criteria
    [ -z "$issue" ]  || printf 'issue: %s\n' "$issue"
    [ -z "$intent" ] || printf 'intent: %s\n' "$intent"
    if [ -n "$exempt" ]; then printf 'exemption: %s\nexemption_because: "%s"\n' "$exempt" "$(printf '%s' "$because" | sed 's/"/\\"/g')"; fi
    [ -z "$bind_standing" ] || printf 'binding: %s\n' "$bind_standing"
    [ -z "$plan_rev" ]      || printf 'plan_revision: %s\n' "$plan_rev"
    [ -z "$ev_standing" ]   || printf 'evidence_standing: %s\n' "$ev_standing"
    printf 'started_at: %s\ncheckpoint_at: %s\noutcome: active\n' "$now" "$now"
    printf '# computed from git; never authored\nrepository_id: %s\nworktree: %s\nbranch: %s\nhead: %s\nworking_tree: %s\n' \
      "$(mj_git_repo_id)" "$MJ_ROOT" "$(mj_git_branch)" "$(mj_git_head)" "$(mj_git_dirty)"
  } > "$MJ_STATE_DIR/current.yaml.mj-tmp" && mv "$MJ_STATE_DIR/current.yaml.mj-tmp" "$MJ_STATE_DIR/current.yaml"
  mj_ledger_append task.started "\"task_id\":\"$id\",\"profile\":\"$profile\",\"owner\":\"$(mj_json_esc "$owner")\",\"scope\":\"$(mj_json_esc "$(printf '%s' "$norm" | sed 's/^ //')")\",\"requires\":\"$(mj_json_esc "$(printf '%s' "$req_norm" | sed 's/^ //')")\"$(
    [ "$bind_asked" = 0 ] || printf ',"issue":"%s","intent":"%s","exemption":"%s","because":"%s","binding":"%s","plan_revision":"%s"' \
      "$(mj_json_esc "$issue")" "$(mj_json_esc "$intent")" "$(mj_json_esc "$exempt")" "$(mj_json_esc "$because")" "${bind_standing:-unknown}" "$plan_rev")"

  printf 'started %s  profile=%s  scope=%s%s\n' "$id" "$profile" "$(printf '%s' "$norm" | sed 's/^ //; s/ /,/g')" \
    "$([ -z "$req_norm" ] || printf '  requires=%s' "$(printf '%s' "$req_norm" | sed 's/^ //; s/ /,/g')")"
  if [ -n "$bind_standing" ]; then
    printf 'binding  %s%s%s\n' "$bind_standing" \
      "$(mj_binding_get '[.intents[]?.id] | if length > 0 then "  serves " + join(", ") else "" end')" \
      "$(mj_binding_get 'if .exemption then "  " + .exemption.class + " — " + .exemption.because else "" end')"
    mj_binding_get '.notes[]? | "note     " + .'
  fi
  mj_report_overlap "$norm"
  mj_mesh_task_claim "$id" "$(printf '%s' "$norm" | sed 's/^ //')" "$task" "$issue"
  # continuity: name the prior record this checkout would resolve to, without injecting it
  if mj_resolve_latest "$MJ_STATE_DIR/handovers" ""; then
    mj_info handover "${MJ_RES_PATH#"$MJ_ROOT/"}" \
      "prior record, $MJ_RES_MATCH, $(mj_git_label "$MJ_RES_HEAD" "$MJ_RES_BRANCH"), $(mj_age_human "$(mj_age_minutes "$MJ_RES_CREATED" || true)")" \
      "majordomus handover --resolve"
  fi
  printf 'next: majordomus context; checkpoint every %s; majordomus check before claiming anything\n' "$(mj_pro checkpoint_interval)"
}

# report claims in other worktrees that contain or are contained by our scope
mj_report_overlap() {
  local mine="$1" wt other oflat q ol
  mj_git worktree list --porcelain 2>/dev/null | sed -n 's/^worktree //p' | while read -r wt; do
    [ "$wt" = "$MJ_ROOT" ] && continue
    other="$wt/$(mj_rel "$MJ_STATE_DIR")/current.yaml"; [ -f "$other" ] || continue
    oflat="$(mktemp "${TMPDIR:-/tmp}/mj.ov.XXXXXX")"; mj_yaml_flatten "$other" > "$oflat" 2>/dev/null || { rm -f "$oflat"; continue; }
    [ "$(mj_yget "$oflat" outcome)" = active ] || { rm -f "$oflat"; continue; }
    for ol in $(mj_ylist "$oflat" scope); do for q in $mine; do
      if mj_path_contains "$ol" "$q"; then mj_info overlap "$(basename "$wt")" "claims $ol — contains your $q" "majordomus check --overlap"
      elif mj_path_contains "$q" "$ol"; then mj_info overlap "$(basename "$wt")" "claims $ol — contained by your $q" "majordomus check --overlap"; fi
    done; done
    rm -f "$oflat"
  done
}

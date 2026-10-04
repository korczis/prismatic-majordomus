#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_start:-}" ] && return 0 || MJ_LIB_start=1
# start — begin a scoped task under a profile. One active task per checkout.
# shellcheck source=handover.sh
. "$MJ_LIB_DIR/handover.sh"
# shellcheck source=check.sh
. "$MJ_LIB_DIR/check.sh"
mj_cmd_start() {
  local task="" scope="" requires="" profile="" owner="${USER:-unknown}" issue=""
  while [ $# -gt 0 ]; do case "$1" in
    --scope) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--scope needs paths"; scope="$scope,$2"; shift 2 ;;
    --scope=*) scope="$scope,${1#--scope=}"; shift ;;
    --requires) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--requires needs obligation tokens"; requires="$requires,$2"; shift 2 ;;
    --requires=*) requires="$requires,${1#--requires=}"; shift ;;
    --profile) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--profile needs a name"; profile="$2"; shift 2 ;;
    --profile=*) profile="${1#--profile=}"; shift ;;
    --issue) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--issue needs an issue id, or none"; issue="$2"; shift 2 ;;
    --issue=*) issue="${1#--issue=}"; shift ;;
    --owner) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--owner needs a value"; owner="$2"; shift 2 ;;
    --help|-h) cat <<H
usage: majordomus start "<task>" --scope <path>[,<path>...] [--requires <token>[,<token>...]]
                       [--profile <name>] [--owner <who>] [--issue <id>|none]
  one active task per checkout; refuses (15) while a task is active — handover or finish it first
  scope paths are normalised (no trailing /, no escapes); overlap with other worktrees is reported
  --requires  what this task owes before it may be called completed: tokens of
              share/obligations.yaml, listed by \`majordomus evidence\` with no arguments.
              A scope says where a worker may write; requires says what the worker owes.
              \`check\` reports each one, \`evidence --covers\` discharges the ones a worker
              records, and \`finish --outcome completed\` refuses while any is outstanding.
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

  local id; id="t-$(mj_now_compact | tr -d 'TZ')-$(mj_rand16 | cut -c1-4)"
  local now; now="$(mj_now)"
  mkdir -p "$MJ_STATE_DIR"
  {
    printf 'id: %s\ntask: "%s"\nprofile: %s\nowner: "%s"\nscope:\n' "$id" "$(printf '%s' "$task" | sed 's/"/\\"/g')" "$profile" "$owner"
    for p in $norm; do printf '  - %s\n' "$p"; done
    if [ -n "$req_norm" ]; then printf 'requires:\n'; for tok in $req_norm; do printf '  - %s\n' "$tok"; done; fi
    [ -n "$issue" ] && printf 'issue: %s\n' "$issue"
    printf 'started_at: %s\ncheckpoint_at: %s\noutcome: active\n' "$now" "$now"
    printf '# computed from git; never authored\nrepository_id: %s\nworktree: %s\nbranch: %s\nhead: %s\nworking_tree: %s\n' \
      "$(mj_git_repo_id)" "$MJ_ROOT" "$(mj_git_branch)" "$(mj_git_head)" "$(mj_git_dirty)"
  } > "$MJ_STATE_DIR/current.yaml.mj-tmp" && mv "$MJ_STATE_DIR/current.yaml.mj-tmp" "$MJ_STATE_DIR/current.yaml"
  mj_ledger_append task.started "\"task_id\":\"$id\",\"profile\":\"$profile\",\"owner\":\"$(mj_json_esc "$owner")\",\"scope\":\"$(mj_json_esc "$(printf '%s' "$norm" | sed 's/^ //')")\",\"requires\":\"$(mj_json_esc "$(printf '%s' "$req_norm" | sed 's/^ //')")\""

  printf 'started %s  profile=%s  scope=%s%s\n' "$id" "$profile" "$(printf '%s' "$norm" | sed 's/^ //; s/ /,/g')" \
    "$([ -z "$req_norm" ] || printf '  requires=%s' "$(printf '%s' "$req_norm" | sed 's/^ //; s/ /,/g')")"
  mj_report_overlap "$norm"
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

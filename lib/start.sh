#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_start:-}" ] && return 0 || MJ_LIB_start=1
# start — begin a scoped task under a profile. One active task per checkout.
# shellcheck source=handover.sh
. "$MJ_LIB_DIR/handover.sh"
# shellcheck source=check.sh
. "$MJ_LIB_DIR/check.sh"
mj_cmd_start() {
  local task="" scope="" profile="" owner="${USER:-unknown}"
  while [ $# -gt 0 ]; do case "$1" in
    --scope) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--scope needs paths"; scope="$scope,$2"; shift 2 ;;
    --scope=*) scope="$scope,${1#--scope=}"; shift ;;
    --profile) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--profile needs a name"; profile="$2"; shift 2 ;;
    --profile=*) profile="${1#--profile=}"; shift ;;
    --owner) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--owner needs a value"; owner="$2"; shift 2 ;;
    --help|-h) cat <<H
usage: majordomus start "<task>" --scope <path>[,<path>...] [--profile <name>] [--owner <who>]
  one active task per checkout; refuses (15) while a task is active — handover or finish it first
  scope paths are normalised (no trailing /, no escapes); overlap with other worktrees is reported
H
      return 0 ;;
    -*) mj_die "$MJ_EX_USAGE" "start: unknown option $1" ;;
    *) [ -z "$task" ] || mj_die "$MJ_EX_USAGE" "start: task must be one argument (quote it)"; task="$1"; shift ;;
  esac; done
  [ -n "$task" ]  || mj_die "$MJ_EX_USAGE" "start: a task description is required"
  scope="${scope#,}"
  [ -n "$(printf '%s' "$scope" | tr -d ', ')" ] || mj_die "$MJ_EX_USAGE" "start: --scope is required (comma-separated repository paths)"
  mj_require_installed
  mj_load_policy || mj_die "$MJ_EX_CONTRACT" "policy does not parse (run: majordomus doctor)"
  [ -n "$profile" ] || profile="$(mj_pol profiles.default)"
  mj_load_profile "$profile" || mj_die "$MJ_EX_MISSING" "no profile '$profile' (.majordomus/profiles/$profile.yaml)"

  # existing task? One active task per checkout — so a record belonging to another checkout
  # does not block this one. It is replaced in this working copy and left alone everywhere
  # else, and the hazard of committing that replacement is stated rather than hidden.
  if mj_load_current; then
    local oc; oc="$(mj_cur outcome)"
    if mj_task_is_foreign; then
      mj_warn task "$(mj_cur id)" "the record here belongs to $(mj_cur worktree); replacing it in this working copy only" "git diff .majordomus/state/current.yaml"
      [ "$oc" = active ] && mj_warn task "$(mj_cur id)" "that task is still active there; committing this file would replace its record on the branch" "cat .majordomus/state/current.yaml"
    else
      case "$oc" in
        active) mj_die "$MJ_EX_REFUSED" "task $(mj_cur id) is active ('$(mj_cur task)'); run majordomus handover or majordomus finish first" ;;
        *) mkdir -p "$MJ_DIR/state/archive"; mv "$MJ_CUR" "$MJ_DIR/state/archive/$(mj_cur id).yaml" ;;
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

  local id; id="t-$(mj_now_compact | tr -d 'TZ')-$(mj_rand16 | cut -c1-4)"
  local now; now="$(mj_now)"
  {
    printf 'id: %s\ntask: "%s"\nprofile: %s\nowner: "%s"\nscope:\n' "$id" "$(printf '%s' "$task" | sed 's/"/\\"/g')" "$profile" "$owner"
    for p in $norm; do printf '  - %s\n' "$p"; done
    printf 'started_at: %s\ncheckpoint_at: %s\noutcome: active\n' "$now" "$now"
    printf '# computed from git; never authored\nrepository_id: %s\nworktree: %s\nbranch: %s\nhead: %s\nworking_tree: %s\n' \
      "$(mj_git_repo_id)" "$MJ_ROOT" "$(mj_git_branch)" "$(mj_git_head)" "$(mj_git_dirty)"
  } > "$MJ_DIR/state/current.yaml.mj-tmp" && mv "$MJ_DIR/state/current.yaml.mj-tmp" "$MJ_DIR/state/current.yaml"
  mj_ledger_append task.started "\"task_id\":\"$id\",\"profile\":\"$profile\",\"owner\":\"$(mj_json_esc "$owner")\",\"scope\":\"$(mj_json_esc "$(printf '%s' "$norm" | sed 's/^ //')")\""

  printf 'started %s  profile=%s  scope=%s\n' "$id" "$profile" "$(printf '%s' "$norm" | sed 's/^ //; s/ /,/g')"
  mj_report_overlap "$norm"
  # continuity: name the prior record this checkout would resolve to, without injecting it
  if mj_resolve_latest "$MJ_DIR/state/handovers" ""; then
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
    other="$wt/.majordomus/state/current.yaml"; [ -f "$other" ] || continue
    oflat="$(mktemp "${TMPDIR:-/tmp}/mj.ov.XXXXXX")"; mj_yaml_flatten "$other" > "$oflat" 2>/dev/null || { rm -f "$oflat"; continue; }
    [ "$(mj_yget "$oflat" outcome)" = active ] || { rm -f "$oflat"; continue; }
    for ol in $(mj_ylist "$oflat" scope); do for q in $mine; do
      if mj_path_contains "$ol" "$q"; then mj_info overlap "$(basename "$wt")" "claims $ol — contains your $q" "majordomus check --overlap"
      elif mj_path_contains "$q" "$ol"; then mj_info overlap "$(basename "$wt")" "claims $ol — contained by your $q" "majordomus check --overlap"; fi
    done; done
    rm -f "$oflat"
  done
}

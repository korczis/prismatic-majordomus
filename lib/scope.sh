#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_scope:-}" ] && return 0 || MJ_LIB_scope=1
# scope widen — add paths to the active task's scope, keeping its id (I2114).
#
# A scope is what a worker means to change, said before the work. Work finds what the claim
# did not foresee: a gate that wants a roadmap line, an advisory that wants a lock bumped.
# Until this command the only route was `handover --close` and a new `start`, which costs a
# task id and a handover record and which nobody finds without being told; the other route,
# editing current.yaml by hand, is the one the record must never need. A widening is said
# out loud instead: the paths join the scope, the record keeps who widened it, when and why,
# and the ledger keeps every widening. Nothing is ever narrowed here — a path a task has
# touched stays claimed.
#
# `majordomus scope` with any other word is the executable's (the repository scope, read
# only); bin/majordomus hands this one word to the shell (mj_words_are_the_shells).
# shellcheck source=start.sh
. "$MJ_LIB_DIR/start.sh"

mj_cmd_scope() {
  local sub="${1:-}"
  case "$sub" in
    widen) shift; mj_scope_widen "$@" ;;
    *) mj_die "$MJ_EX_USAGE" "scope: the shell tool answers only 'scope widen'; '$sub' is not it" ;;
  esac
}

mj_scope_widen() {
  local paths="" because="" by="${USER:-unknown}"
  while [ $# -gt 0 ]; do case "$1" in
    --because) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--because needs a reason"; because="$2"; shift 2 ;;
    --because=*) because="${1#--because=}"; shift ;;
    --by) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--by needs a value"; by="$2"; shift 2 ;;
    --by=*) by="${1#--by=}"; shift ;;
    --help|-h) cat <<H
usage: majordomus scope widen <path>[,<path>...] [<path>...] --because "<reason>" [--by <who>]
  adds paths to the active task's scope; the task keeps its id
  paths are admitted by the same rule as start --scope; a path the scope already
  contains is left out, and a call that adds nothing writes nothing (exit 0)
  the record keeps who widened it, when and why (scope_widened_*); the ledger keeps
  every widening (task.scope_widened); on a running mesh the task's claim is renewed
  over the whole scope
  refuses (12) when this checkout has no active task of its own
H
      return 0 ;;
    -*) mj_die "$MJ_EX_USAGE" "scope widen: unknown option $1" ;;
    *) paths="$paths,$1"; shift ;;
  esac; done
  paths="${paths#,}"
  [ -n "$(printf '%s' "$paths" | tr -d ', ')" ] || mj_die "$MJ_EX_USAGE" "scope widen: name the paths to add"
  because="$(printf '%s' "$because" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//')"
  [ -n "$because" ] || mj_die "$MJ_EX_USAGE" "scope widen: --because is required; a widening nobody can explain is the scope creep the scope exists to show"
  mj_require_installed
  mj_load_policy || mj_die "$MJ_EX_CONTRACT" "policy does not parse (run: majordomus doctor)"

  mj_load_current || mj_die "$MJ_EX_MISSING" "scope widen: no task here to widen (run: majordomus start)"
  mj_task_is_foreign && mj_die "$MJ_EX_MISSING" "scope widen: the task record here belongs to $(mj_cur worktree); widen it there"
  [ "$(mj_cur outcome)" = active ] || mj_die "$MJ_EX_MISSING" "scope widen: task $(mj_cur id) is $(mj_cur outcome); only an active task's scope can be widened"

  local id old added="" p s covered
  id="$(mj_cur id)"
  old="$(mj_ylist "$MJ_CUR_FLAT" scope | tr '\n' ' ')"
  mj_scope_normalise "scope widen" "$paths"
  for p in $MJ_SCOPE_NORM; do
    covered=0
    for s in $old $added; do mj_path_contains "$s" "$p" && { covered=1; break; }; done
    if [ "$covered" = 1 ]; then
      mj_info scope "$p" "already within the scope of $id; left out" "majordomus check"
    else
      added="$added $p"
    fi
  done
  added="${added# }"
  if [ -z "$added" ]; then
    printf 'unchanged %s  nothing to add; scope=%s\n' "$id" "$(printf '%s' "$old" | sed 's/ $//; s/ /,/g')"
    return 0
  fi

  local now key tmp
  now="$(mj_now)"
  key="$(mj_cur mesh_claim)"
  tmp="$MJ_STATE_DIR/current.yaml.mj-tmp"
  # The record is rewritten whole and moved into place, as start writes it. The new entries
  # follow the old ones under `scope:`; the provenance keys of an earlier widening are
  # replaced (the ledger keeps every one); the claim key is dropped, because the claim is
  # renewed over the whole scope below and writes its own.
  awk -v added="$added" -v by="$(printf '%s' "$by" | sed 's/\\/\\\\/g; s/"/\\"/g')" \
      -v because="$(printf '%s' "$because" | sed 's/\\/\\\\/g; s/"/\\"/g')" -v now="$now" '
    function provenance() {
      printf "scope_widened_at: %s\nscope_widened_by: \"%s\"\nscope_widened_because: \"%s\"\n", now, by, because
      done_prov = 1
    }
    in_scope && !/^  - / {
      n = split(added, a, " "); for (i = 1; i <= n; i++) printf "  - %s\n", a[i]
      in_scope = 0
    }
    /^scope:$/ { print; in_scope = 1; next }
    /^scope_widened_(at|by|because): / { next }
    /^mesh_claim: / { next }
    /^started_at: / && !done_prov { provenance() }
    { print }
    END {
      if (in_scope) { n = split(added, a, " "); for (i = 1; i <= n; i++) printf "  - %s\n", a[i] }
      if (!done_prov) provenance()
    }
  ' "$MJ_CUR" > "$tmp" && mv "$tmp" "$MJ_CUR"

  mj_ledger_append task.scope_widened "\"task_id\":\"$id\",\"scope\":\"$(mj_json_esc "$added")\",\"widened_by\":\"$(mj_json_esc "$by")\",\"because\":\"$(mj_json_esc "$because")\""

  local whole="$old $added"
  whole="$(printf '%s' "$whole" | tr -s ' ' | sed 's/^ //; s/ $//')"
  printf 'widened %s  added=%s  scope=%s\n' "$id" "$(printf '%s' "$added" | sed 's/ /,/g')" "$(printf '%s' "$whole" | sed 's/ /,/g')"
  mj_report_overlap " $added"
  # On a running mesh the task's claim covers its whole scope again: the old claim is
  # released and a new one made, so no linked runtime keeps reading the narrower claim.
  # Silent, and never a failure, when no server answers — check claims it once one does.
  if [ -n "$key" ] && mj_mesh_task_bin; then
    "$MJ_MESH_BIN" mesh release "$key" --repo "$MJ_ROOT" >/dev/null 2>&1 || true
  fi
  mj_mesh_task_claim "$id" "$whole" "$(mj_cur task)" "$(mj_cur issue)"
  return 0
}

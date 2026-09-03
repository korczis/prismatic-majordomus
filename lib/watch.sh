#!/usr/bin/env bash
# watch — what has drifted? Read-only. Exit 11 when any drift is found, 0 otherwise.
# shellcheck source=check.sh
. "$MJ_LIB_DIR/check.sh"
mj_cmd_watch() {
  local a; for a in "$@"; do case "$a" in
    --help|-h) echo "usage: majordomus watch [--json]   (read-only; exit 0 no drift, 11 drift found)"; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "watch: unknown option $a" ;; esac; done
  mj_require_installed
  mj_load_policy || mj_die "$MJ_EX_CONTRACT" "policy does not parse (run: majordomus doctor)"

  # policy + projection drift via fingerprints
  local fp="$MJ_DIR/generated/fingerprints.yaml" fpflat="" tmp psha
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.w.XXXXXX")"; cat "$MJ_DIR/policy.yaml" "$MJ_DIR"/profiles/*.yaml > "$tmp"; psha="$(mj_sha256 "$tmp")"; rm -f "$tmp"
  if [ -f "$fp" ]; then
    fpflat="$(mktemp "${TMPDIR:-/tmp}/mj.wf.XXXXXX")"; mj_yaml_flatten "$fp" > "$fpflat" 2>/dev/null || { rm -f "$fpflat"; fpflat=""; }
  fi
  if [ -z "$fpflat" ]; then mj_drift policy "fingerprints" "no projections generated yet" "majordomus update"
  else
    if [ "$(mj_yget "$fpflat" policy_sha256)" = "$psha" ]; then mj_ok policy "policy+profiles" "match last update (${psha:0:12})"
    else mj_drift policy ".majordomus/policy.yaml" "policy or profiles changed after the last update" "majordomus update --dry-run"; fi
    local k=0 tgt
    while [ -n "$(mj_yget "$fpflat" "targets.$k.target")" ]; do
      tgt="$(mj_yget "$fpflat" "targets.$k.target")"
      if [ ! -f "$MJ_ROOT/$tgt" ]; then mj_drift projection "$tgt" "missing" "majordomus update"
      elif [ "$(mj_sha256 "$MJ_ROOT/$tgt")" != "$(mj_yget "$fpflat" "targets.$k.sha256")" ]; then mj_drift projection "$tgt" "hash differs from fingerprint (hand-edited?)" "majordomus update --diff $tgt"
      else mj_ok projection "$tgt" "matches fingerprint"; fi
      k=$((k+1))
    done
    rm -f "$fpflat"
  fi

  # state, scope, staleness, verification
  if mj_load_current; then
    local id label; id="$(mj_cur id)"; label="$(mj_git_label "$(mj_cur head)" "$(mj_cur branch)")"
    case "$label" in
      exact|advanced) mj_ok state "$id" "$label" ;;
      *) mj_drift state "$id" "$label — record no longer describes this checkout" "majordomus check" ;;
    esac
    mj_load_profile "$(mj_cur profile)" 2>/dev/null || mj_drift state "$id" "profile '$(mj_cur profile)' has no file"
    case "$(mj_cur outcome)" in
      active)
        local f s inside out=0
        for f in $(mj_git_touched "$(mj_cur head)"); do
          case "$f" in .majordomus/*) continue ;; esac; case " $(mj_projection_targets | tr '\n' ' ') " in *" $f "*) continue ;; esac
          inside=0; for s in $(mj_ylist "$MJ_CUR_FLAT" scope); do mj_path_contains "$s" "$f" && { inside=1; break; }; done
          [ "$inside" = 1 ] || { out=$((out+1)); mj_drift scope "$f" "outside claimed scope" "majordomus check"; }
        done
        [ "$out" = 0 ] && mj_ok scope "$id" "touched files within scope"
        local interval cp_e age; interval="$(mj_duration_secs "$(mj_pro checkpoint_interval 2>/dev/null || echo 15m)")" || interval=900
        cp_e="$(mj_epoch "$(mj_cur checkpoint_at)")"; age=$(( $(mj_epoch "$(mj_now)") - ${cp_e:-0} ))
        if [ "$age" -gt "$interval" ]; then mj_drift staleness "$id" "last checkpoint $((age/60))m ago, interval $(mj_pro checkpoint_interval)" "majordomus check --checkpoint"
        else mj_ok staleness "$id" "checkpoint $((age/60))m ago"; fi ;;
      completed|partial|blocked|no_match|failed)
        if grep -q "\"event\":\"task.finished\".*\"task_id\":\"$id\"" "$MJ_DIR/state/ledger.jsonl" 2>/dev/null; then mj_ok verification "$id" "$(mj_cur outcome) with a finish record"
        else mj_drift verification "$id" "marked $(mj_cur outcome) but no task.finished record in the ledger" "grep task.finished .majordomus/state/ledger.jsonl"; fi ;;
      handed_over)
        local hv; hv="$(grep -l "^task_id: $id$" "$MJ_DIR"/state/handovers/*.md 2>/dev/null | sort | tail -n1)"
        if [ -z "$hv" ]; then mj_drift handover "$id" "marked handed_over but no handover file names it" "ls .majordomus/state/handovers"
        else local miss; miss="$(mj_check_sections "$hv" "$(mj_ylist "$MJ_POL_FLAT" handover.required_sections | tr '\n' '|')")"
          if [ -z "$miss" ]; then mj_ok handover "$id" "$(basename "$hv")"; else mj_drift handover "$(basename "$hv")" "missing section(s): $miss" "grep -n '^# ' $hv"; fi; fi ;;
      *) mj_drift state "$id" "unknown outcome '$(mj_cur outcome)'" "cat .majordomus/state/current.yaml" ;;
    esac
  else mj_info state "-" "no active task"; fi

  # retention
  local cap n
  cap="$(mj_pol ledger.retention_max_lines)"; n=0; [ -f "$MJ_DIR/state/ledger.jsonl" ] && n="$(mj_lines "$MJ_DIR/state/ledger.jsonl")"
  if [ "$n" -le "${cap:-5000}" ]; then mj_ok retention "ledger" "$n lines"; else mj_drift retention "ledger" "$n lines over cap $cap" "wc -l .majordomus/state/ledger.jsonl"; fi
  cap="$(mj_pol handover.retention_max_files)"; n="$(find "$MJ_DIR/state/handovers" -name '*.md' 2>/dev/null | wc -l | tr -d ' ')"
  if [ "$n" -le "${cap:-200}" ]; then mj_ok retention "handovers" "$n files"; else mj_drift retention "handovers" "$n files over cap $cap" "ls .majordomus/state/handovers | wc -l"; fi

  [ "$MJ_JSON" = 1 ] || printf 'watch: %s drift finding(s)\n' "$MJ_FAILS"
  [ "$MJ_FAILS" = 0 ] && exit "$MJ_EX_OK" || exit "$MJ_EX_DRIFT"
}
# handover.sh provides mj_check_sections
# shellcheck source=handover.sh
. "$MJ_LIB_DIR/handover.sh"

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
  local fp="$MJ_DIR/generated/fingerprints.yaml" fpflat="" tmp psha owned
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.w.XXXXXX")"; mj_policy_cat > "$tmp"; psha="$(mj_sha256 "$tmp")"; rm -f "$tmp"
  owned="$(mktemp "${TMPDIR:-/tmp}/mj.wo.XXXXXX")"
  if [ -f "$fp" ]; then
    fpflat="$(mktemp "${TMPDIR:-/tmp}/mj.wf.XXXXXX")"; mj_yaml_flatten "$fp" > "$fpflat" 2>/dev/null || { rm -f "$fpflat"; fpflat=""; }
  fi
  if [ -z "$fpflat" ]; then mj_drift policy "fingerprints" "no projections generated yet" "majordomus update"
  else
    if [ "$(mj_yget "$fpflat" policy_sha256)" = "$psha" ]; then mj_ok policy "policy+profiles" "match last update (${psha:0:12})"
    else mj_drift policy ".majordomus/policy.yaml" "policy or profiles changed after the last update" "majordomus update --dry-run"; fi
    local k=0 tgt mode rc
    while [ -n "$(mj_yget "$fpflat" "targets.$k.target")" ]; do
      tgt="$(mj_yget "$fpflat" "targets.$k.target")"
      mode="$(mj_yget "$fpflat" "targets.$k.mode")"; [ -n "$mode" ] || mode="file"
      if [ ! -f "$MJ_ROOT/$tgt" ]; then mj_drift projection "$tgt" "missing" "majordomus update"
      elif [ "$mode" = region ]; then
        # only the region is Majordomus's; an edit outside the markers is not drift
        rc=0; mj_region_extract "$MJ_ROOT/$tgt" > "$owned" 2>/dev/null || rc=$?
        if [ "$rc" = 1 ]; then mj_drift projection "$tgt" "region markers are absent" "majordomus update"
        elif [ "$rc" != 0 ]; then mj_drift projection "$tgt" "region markers are malformed" "grep -n 'majordomus:begin' $tgt"
        elif [ "$(mj_sha256 "$owned")" != "$(mj_yget "$fpflat" "targets.$k.sha256")" ]; then mj_drift projection "$tgt" "region differs from fingerprint (hand-edited?)" "majordomus update --diff $tgt"
        else mj_ok projection "$tgt" "region matches fingerprint"; fi
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
    local have_profile=1
    mj_load_profile "$(mj_cur profile)" 2>/dev/null || { have_profile=0; mj_drift state "$id" "profile '$(mj_cur profile)' has no file"; }
    case "$(mj_cur outcome)" in
      active)
        local f s inside out=0
        for f in $(mj_git_touched "$(mj_cur head)"); do
          case "$f" in .majordomus/*) continue ;; esac; case " $(mj_projection_targets | tr '\n' ' ') " in *" $f "*) continue ;; esac
          inside=0; for s in $(mj_ylist "$MJ_CUR_FLAT" scope); do mj_path_contains "$s" "$f" && { inside=1; break; }; done
          [ "$inside" = 1 ] || { out=$((out+1)); mj_drift scope "$f" "outside claimed scope" "majordomus check"; }
        done
        [ "$out" = 0 ] && mj_ok scope "$id" "touched files within scope"
        local interval iv cp_e age
        iv=15m; [ "$have_profile" = 1 ] && iv="$(mj_pro checkpoint_interval)"; [ -n "$iv" ] || iv=15m
        interval="$(mj_duration_secs "$iv")" || interval=900
        cp_e="$(mj_epoch "$(mj_cur checkpoint_at)")"
        if [ -z "$cp_e" ]; then mj_drift staleness "$id" "checkpoint_at is missing or unparseable" "grep -n checkpoint_at .majordomus/state/current.yaml"
        else
          age=$(( $(mj_epoch "$(mj_now)") - cp_e ))
          if [ "$age" -gt "$interval" ]; then mj_drift staleness "$id" "last checkpoint $((age/60))m ago, interval $iv" "majordomus check --checkpoint"
          else mj_ok staleness "$id" "checkpoint $((age/60))m ago"; fi
        fi ;;
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

  # continuity drift: records that no longer describe this checkout, and stores that stopped
  # parsing. Every finding names the command that reproduces it.
  mj_watch_continuity

  # retention
  local cap n
  cap="$(mj_pol ledger.retention_max_lines)"; n=0; [ -f "$MJ_DIR/state/ledger.jsonl" ] && n="$(mj_lines "$MJ_DIR/state/ledger.jsonl")"
  if [ "$n" -le "${cap:-5000}" ]; then mj_ok retention "ledger" "$n lines"; else mj_drift retention "ledger" "$n lines over cap $cap" "wc -l .majordomus/state/ledger.jsonl"; fi
  cap="$(mj_pol handover.retention_max_files)"; n="$(find "$MJ_DIR/state/handovers" -name '*.md' 2>/dev/null | wc -l | tr -d ' ')"
  if [ "$n" -le "${cap:-200}" ]; then mj_ok retention "handovers" "$n files"; else mj_drift retention "handovers" "$n files over cap $cap" "majordomus handover --list | wc -l"; fi
  cap="$(mj_pol checkpoint.retention_max_files)"; n="$(find "$MJ_DIR/state/checkpoints" -name '*.md' 2>/dev/null | wc -l | tr -d ' ')"
  if [ "$n" -le "${cap:-500}" ]; then mj_ok retention "checkpoints" "$n files"; else mj_drift retention "checkpoints" "$n files over cap $cap" "majordomus checkpoint --list | wc -l"; fi

  rm -f "$owned"
  [ "$MJ_JSON" = 1 ] || printf 'watch: %s drift finding(s)\n' "$MJ_FAILS"
  [ "$MJ_FAILS" = 0 ] && exit "$MJ_EX_OK" || exit "$MJ_EX_DRIFT"
}
# handover.sh provides mj_check_sections; the rest provide the store validators and builder
# shellcheck source=handover.sh
. "$MJ_LIB_DIR/handover.sh"
# shellcheck source=question.sh
. "$MJ_LIB_DIR/question.sh"
# shellcheck source=decision.sh
. "$MJ_LIB_DIR/decision.sh"
# shellcheck source=prompt.sh
. "$MJ_LIB_DIR/prompt.sh"
# shellcheck source=context.sh
. "$MJ_LIB_DIR/context.sh"

mj_watch_continuity() {
  local bad

  # stores the gates read must keep parsing; a gate that cannot read an entry is bypassed
  bad="$(mj_question_malformed "$MJ_DIR/state/open-questions.md")"
  if [ -n "$bad" ]; then mj_drift records "open-questions.md" "line(s) $(printf '%s' "$bad" | sed 's/ $//') do not parse" "majordomus question list --all"
  else mj_ok records "open-questions.md" "parses"; fi
  bad="$(mj_ledger_bad_lines "$MJ_DIR/state/ledger.jsonl")"
  if [ -n "$bad" ]; then mj_drift records "ledger.jsonl" "line(s) $(printf '%s' "$bad" | sed 's/ $//') are not well-formed events" "majordomus history --validate"
  else mj_ok records "ledger.jsonl" "parses"; fi
  bad="$(mj_decision_malformed "$MJ_DIR/state/decisions.md")"
  [ -n "$bad" ] && mj_drift records "decisions.md" "entry at line(s) $(printf '%s' "$bad" | sed 's/ $//') lacks Task, Head or Why" "majordomus decision list"

  # a prompt asset that no longer renders is a broken reference, not a cosmetic problem
  local f n=0 reason
  if [ -d "$MJ_DIR/prompts" ]; then
    for f in "$MJ_DIR"/prompts/*.md; do
      [ -f "$f" ] || continue; n=$((n + 1))
      reason="$(mj_prompt_validate "$f")" || mj_drift prompt "$(basename "$f" .md)" "$(printf '%s' "$reason" | tr '\n' ';' | sed 's/;$//')" "majordomus prompt show $(basename "$f" .md)"
    done
    [ "$n" = 0 ] && mj_drift prompt ".majordomus/prompts" "no assets; the projected instructions point workers at prompt list" "majordomus update"
  fi

  # the newest handover this checkout resolves to, against the git state it recorded
  if mj_resolve_latest "$MJ_DIR/state/handovers" ""; then
    local label; label="$(mj_git_label "$MJ_RES_HEAD" "$MJ_RES_BRANCH")"
    case "$label" in
      diverged|different_context)
        mj_drift handover "$(basename "$MJ_RES_PATH")" "$label — the newest record for this branch describes a history this checkout no longer has" "majordomus handover --resolve" ;;
      *) mj_ok handover "$(basename "$MJ_RES_PATH")" "$label, $(mj_age_human "$(mj_age_minutes "$MJ_RES_CREATED" || true)")" ;;
    esac
  fi
  [ "${MJ_RES_SKIPPED:-0}" -gt 0 ] && mj_drift handover "handovers" "$MJ_RES_SKIPPED record(s) skipped as malformed" "majordomus handover --list"

  # a checkpoint older than the active task's interval, when one exists at all
  if mj_load_current 2>/dev/null && [ "$(mj_cur outcome)" = active ]; then
    local id; id="$(mj_cur id)"
    if mj_resolve_latest "$MJ_DIR/state/checkpoints" "$id"; then
      mj_ok checkpoint "$(basename "$MJ_RES_PATH")" "$(mj_age_human "$(mj_age_minutes "$MJ_RES_CREATED" || true)")"
    else
      mj_info checkpoint "$id" "no checkpoint record; only checkpoint_at is set" "majordomus checkpoint"
    fi
  fi

  # the assembled context must fit its own budget, through the real command path
  local budget out lines
  budget="$(mj_pol context.builder_budget_lines)"; [ -n "$budget" ] || budget=300
  out="$(mktemp "${TMPDIR:-/tmp}/mj.wc.XXXXXX")"
  if ( export MJ_JSON=0; mj_cmd_context ) > "$out" 2>/dev/null; then
    lines="$(mj_lines "$out")"
    if [ "$lines" -le "$budget" ]; then mj_ok context "builder" "$lines lines, budget $budget"
    else mj_drift context "builder" "$lines lines over budget $budget" "majordomus context"; fi
  else mj_drift context "builder" "majordomus context failed" "majordomus context"; fi
  rm -f "$out"
}

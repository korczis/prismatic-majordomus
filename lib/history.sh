#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_history:-}" ] && return 0 || MJ_LIB_history=1
# history — read the append-only ledger back. Operational reconstruction, not a transcript:
# what happened, when, for which task, at which git head, and what was accepted.

mj_cmd_history() {
  local want_task="" want_event="" since="" limit=20 rotate=0 validate=0
  while [ $# -gt 0 ]; do case "$1" in
    --task) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--task needs an id"; want_task="$2"; shift 2 ;;
    --task=*) want_task="${1#--task=}"; shift ;;
    --event) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--event needs a name"; want_event="$2"; shift 2 ;;
    --event=*) want_event="${1#--event=}"; shift ;;
    --since) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--since needs a duration or timestamp"; since="$2"; shift 2 ;;
    --since=*) since="${1#--since=}"; shift ;;
    --limit) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--limit needs a number"; limit="$2"; shift 2 ;;
    --limit=*) limit="${1#--limit=}"; shift ;;
    --all) limit=0; shift ;;
    --rotate) rotate=1; shift ;;
    --validate) validate=1; shift ;;
    --help|-h) cat <<H
usage: majordomus history [--task <id>] [--event <name>] [--since <n>m|h|d|<timestamp>]
                          [--limit <n>|--all] [--json]
       majordomus history --validate
       majordomus history --rotate
  reads .ai/local/state/ledger.jsonl, oldest line first, newest --limit lines (default 20)
  --json      the matching ledger lines verbatim, one JSON object per line
  --validate  report malformed ledger lines; exit 10 when any is found
  --rotate    move all but the newest ledger.retention_max_lines lines into
              ledger.<utc>.jsonl.archived; never deletes, refuses when under the cap
H
      return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "history: unknown option $1" ;;
  esac; done
  case "$limit" in ''|*[!0-9]*) mj_die "$MJ_EX_USAGE" "history: --limit must be a number" ;; esac
  mj_require_installed
  local led="$MJ_STATE_DIR/ledger.jsonl"
  if [ ! -f "$led" ]; then
    [ "$MJ_JSON" = 1 ] || printf 'no ledger yet ($(mj_rel "$MJ_STATE_DIR")/ledger.jsonl)\n'
    return 0
  fi
  if [ "$validate" = 1 ]; then mj_history_validate "$led"; return; fi
  if [ "$rotate" = 1 ]; then
    [ -n "${MJ_POL_FLAT:-}" ] || mj_load_policy || mj_die "$MJ_EX_CONTRACT" "policy does not parse (run: majordomus doctor)"
    mj_history_rotate "$led"; return
  fi

  # An unregistered name is refused rather than answered with "no matching events", which
  # would be the same answer a registered name that has not occurred yet produces. A filter
  # that cannot say which of those two it means is not a filter.
  if [ -n "$want_event" ]; then
    mj_events_load
    mj_event_known "$want_event" || mj_die "$MJ_EX_USAGE" \
      "history: unknown event '$want_event' (one of: $(mj_event_ids | tr '\n' ' '))"
  fi

  local cutoff=""
  if [ -n "$since" ]; then
    case "$since" in
      *[0-9][smhd]) local secs; secs="$(mj_since_secs "$since")" || mj_die "$MJ_EX_USAGE" "history: cannot read --since '$since'"
        cutoff="$(mj_epoch_to_iso $(( $(mj_epoch "$(mj_now)") - secs )))" ;;
      *) mj_epoch "$since" >/dev/null 2>&1 || mj_die "$MJ_EX_USAGE" "history: cannot read --since '$since'"; cutoff="$since" ;;
    esac
  fi

  mj_history_render "$led" "$want_task" "$want_event" "$cutoff" "$limit" "$MJ_JSON"
}

# "90m" / "2h" / "7d" -> seconds
mj_since_secs() {
  local v="$1" n u; n="${v%[smhd]}"; u="${v#"$n"}"
  case "$n" in ''|*[!0-9]*) return 1 ;; esac
  case "$u" in s) printf '%s' "$n" ;; m) printf '%s' $((n*60)) ;; h) printf '%s' $((n*3600)) ;; d) printf '%s' $((n*86400)) ;; *) return 1 ;; esac
}
mj_epoch_to_iso() {
  date -u -r "$1" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null && return
  date -u -d "@$1" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null
}

mj_history_validate() {
  local led="$1" bad
  bad="$(mj_ledger_bad_lines "$led")"
  if [ -n "$bad" ]; then
    local n; for n in $bad; do
      mj_fail ledger "line $n" "not a well-formed ledger event (needs ts and event)" "sed -n '${n}p' $(mj_rel "$MJ_STATE_DIR")/ledger.jsonl"
    done
    [ "$MJ_JSON" = 1 ] || printf 'history --validate: %s malformed line(s)\n' "$(printf '%s\n' "$bad" | wc -w | tr -d ' ')"
    exit "$MJ_EX_CONTRACT"
  fi
  # A well-formed line can still carry a name no reader knows. That line is durable and
  # will be silently skipped by every filter, so it is a failure rather than a curiosity.
  mj_events_load
  local unknown="" e n=0
  while IFS= read -r e; do
    n=$((n+1))
    [ -n "$e" ] || continue
    mj_event_known "$e" || case " $unknown " in *" $e "*) ;; *) unknown="$unknown $e" ;; esac
  done < <(sed -n 's/.*"event":"\([^"]*\)".*/\1/p' "$led")
  if [ -n "$unknown" ]; then
    for e in $unknown; do
      mj_fail ledger "$e" "not a registered event; no reader recognises it" "grep -n '\"event\":\"$e\"' .ai/local/state/ledger.jsonl"
    done
    [ "$MJ_JSON" = 1 ] || printf 'history --validate: %s unregistered event name(s)\n' "$(printf '%s\n' $unknown | wc -w | tr -d ' ')"
    exit "$MJ_EX_CONTRACT"
  fi
  mj_ok ledger "$(mj_lines "$led") lines" "every line carries ts and event, and every event is registered"
  [ "$MJ_JSON" = 1 ] || printf 'history --validate: ok\n'
}

mj_history_rotate() {
  local led="$1" cap have keep_from archive
  cap="$(mj_pol_req ledger.retention_max_lines)"
  have="$(mj_lines "$led")"
  if [ "$have" -le "$cap" ]; then
    printf 'nothing to rotate: %s lines, cap %s\n' "$have" "$cap"; return 0
  fi
  archive="$MJ_STATE_DIR/ledger.$(mj_now_compact).jsonl.archived"
  [ -e "$archive" ] && mj_die "$MJ_EX_REFUSED" "archive $archive already exists; nothing written"
  keep_from=$((have - cap + 1))
  head -n $((keep_from - 1)) "$led" > "$archive" || mj_die "$MJ_EX_INTERNAL" "could not write $archive"
  tail -n "$cap" "$led" > "$led.mj-tmp" && mv "$led.mj-tmp" "$led"
  mj_ledger_append ledger.rotated "\"archived\":$((keep_from - 1)),\"kept\":$cap,\"archive\":\"$(mj_json_esc "${archive#"$MJ_ROOT/"}")\""
  printf 'rotated: %s line(s) to %s, %s kept\n' "$((keep_from - 1))" "${archive#"$MJ_ROOT/"}" "$cap"
}

# mj_history_render LEDGER TASK EVENT CUTOFF LIMIT JSON
# Oldest line first. LIMIT 0 means everything. JSON 1 emits the matching lines verbatim.
mj_history_render() {
  awk -v task="$2" -v ev="$3" -v cutoff="$4" -v limit="$5" -v json="$6" '
    function field(s, k,   r) {
      if (match(s, "\"" k "\":\"")) { r = substr(s, RSTART + length(k) + 4); sub(/".*/, "", r); return r }
      if (match(s, "\"" k "\":"))   { r = substr(s, RSTART + length(k) + 3); sub(/[,}].*/, "", r); return r }
      return ""
    }
    {
      ts = field($0, "ts"); e = field($0, "event"); t = field($0, "task_id")
      if (ts == "" || e == "") next
      if (task != "" && t != task) next
      if (ev != "" && e != ev) next
      if (cutoff != "" && ts < cutoff) next
      n++; keep[n] = $0
    }
    END{
      first = (limit > 0 && n > limit) ? n - limit + 1 : 1
      for (i = first; i <= n; i++) {
        s = keep[i]
        if (json == 1) { print s; continue }
        ts = field(s, "ts"); e = field(s, "event"); t = field(s, "task_id"); head = field(s, "head")
        detail = ""
        if (e == "task.started")             detail = "profile=" field(s, "profile") " scope=" field(s, "scope")
        else if (e == "task.finished")       detail = "outcome=" field(s, "outcome") (index(s, "\"verify\":{") ? " verify_exit=" field(s, "exit") : "")
        else if (e == "task.handed_over")  { d = field(s, "handover_path"); sub(/.*\//, "", d); detail = d " closed=" field(s, "closed") }
        else if (e == "task.checkpoint")   { d = field(s, "checkpoint_path"); sub(/.*\//, "", d); detail = (d == "" ? "(no body)" : d) }
        else if (e == "decision.recorded")   detail = field(s, "decision")
        else if (e == "question.opened")     detail = field(s, "question")
        else if (e == "question.resolved")   detail = field(s, "question") " -> " field(s, "answer")
        else if (e == "projections.updated") detail = "targets=" field(s, "targets")
        else if (e == "ledger.rotated")      detail = "archived=" field(s, "archived") " kept=" field(s, "kept")
        printf "%s  %-19s %-22s %-8s %s\n", ts, e, (t == "" ? "-" : t), substr(head, 1, 7), detail
      }
      if (n == 0 && json != 1) print "no matching events"
      if (limit > 0 && n > limit && json != 1) printf "(%s of %s events; --all for everything)\n", limit, n
    }' "$1"
}

#!/usr/bin/env bash
# shellcheck disable=SC2034  # MJ_DOCTRINE_SKIPPED is read by the dispatcher in doctrine.sh
# plan — the milestone and issue model: validate it, read it, and move an issue through it.
#
# Every subcommand reads the model lib/project.sh builds and lib/project.awk derives.
# None of them computes a status, a wave or a ready set of its own; that is the whole
# point of having one engine. The four writing subcommands (start, verify, evidence,
# done) write one lifecycle marker each into the issue's own file and nothing else.

# shellcheck source=project.sh
. "$MJ_LIB_DIR/project.sh"

mj_plan_usage() {
  cat <<'H'
usage: majordomus plan <subcommand> [options]

READ
  validate             schemas, references, the DAG, and status consistency   (read-only)
  status               milestone progress and the next executable issue       (read-only)
  list                 one line per issue: id, status, wave, milestone, title (read-only)
  show <id>            the full record of one milestone or issue              (read-only)
  roadmap              milestones in derived order, with now and next         (read-only)
  rgraph               the milestone dependency graph as Mermaid                (read-only)
  ready                issues whose dependencies are all satisfied            (read-only)
  blocked              issues waiting on a dependency, and on which one       (read-only)
  waves                topological execution waves, derived from the graph    (read-only)
  graph                the dependency DAG as Mermaid                          (read-only)
  next                 the one issue a worker should take now                 (read-only)
  body <id>            the provider-neutral projection body for one record    (read-only)

WRITE (one lifecycle marker each, into the issue's own file)
  start <id>           record that execution began
  verify <id>          record that implementation is complete, evidence pending
  evidence <id>        attach one piece of evidence to an issue or a milestone
  done <id>            record completion; refuses while required evidence is missing

options:
  --json               machine-readable output (read-only subcommands)
  --milestone <id>     restrict list, ready, blocked, waves and graph to one milestone
  --covers <token>     evidence: which evidence_required entry this satisfies
  --type <kind>        evidence: test | build | ci | artifact | manual
  --command <cmd>      evidence: the command that produced it
  --result <text>      evidence: what it showed
  --artifact <ref>     evidence: a path, URL or hash

exit codes: 0 ok · 2 usage · 10 the model is invalid or a transition is refused
            12 no canonical project model in this repository · 15 refused
H
}

mj_cmd_plan() {
  local sub="" m="" covers="" etype="" ecmd="" eres="" eart="" ids=""
  [ $# -ge 1 ] || { mj_plan_usage; exit "$MJ_EX_USAGE"; }
  case "$1" in
    --help|-h|help) mj_plan_usage; return 0 ;;
    validate|status|list|show|ready|blocked|waves|graph|next|body|start|verify|evidence|roadmap|rgraph|done) sub="$1"; shift ;;
    *) mj_die "$MJ_EX_USAGE" "plan: unknown subcommand '$1' (see: majordomus plan --help)" ;;
  esac
  while [ $# -gt 0 ]; do case "$1" in
    --help|-h) mj_plan_usage; return 0 ;;
    --milestone) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--milestone needs an id"; m="$2"; shift 2 ;;
    --milestone=*) m="${1#--milestone=}"; shift ;;
    --covers)   [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--covers needs a token"; covers="$2"; shift 2 ;;
    --type)     [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--type needs a kind"; etype="$2"; shift 2 ;;
    --command)  [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--command needs a command"; ecmd="$2"; shift 2 ;;
    --result)   [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--result needs text"; eres="$2"; shift 2 ;;
    --artifact) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--artifact needs a reference"; eart="$2"; shift 2 ;;
    -*) mj_die "$MJ_EX_USAGE" "plan: unknown option $1" ;;
    *) ids="$ids $1"; shift ;;
  esac; done

  mj_require_installed
  local rc=0; mj_project_load || rc=$?
  case "$rc" in
    1) mj_die "$MJ_EX_MISSING" "no canonical project model here (.majordomus/project/project.yaml)" ;;
    2) mj_die "$MJ_EX_CONTRACT" "the canonical project model does not parse; see the errors above" ;;
  esac
  ids="${ids# }"

  case "$sub" in
    validate) mj_plan_validate ;;
    status)   mj_plan_status "$m" ;;
    list)     mj_plan_list "$m" ;;
    show)     mj_plan_show "$ids" ;;
    ready)    mj_plan_state_list READY "$m" ;;
    blocked)  mj_plan_state_list BLOCKED "$m" ;;
    waves)    mj_plan_waves "$m" ;;
    graph)    mj_project_mermaid "$m" ;;
    roadmap)  mj_plan_roadmap ;;
    rgraph)   mj_project_roadmap_mermaid ;;
    next)     mj_plan_next "$m" ;;
    body)     mj_plan_body "$ids" ;;
    start)    mj_plan_transition "$ids" start ;;
    verify)   mj_plan_transition "$ids" verify ;;
    done)     mj_plan_transition "$ids" "done" ;;
    evidence) mj_plan_evidence "$ids" "$covers" "$etype" "$ecmd" "$eres" "$eart" ;;
  esac
}

# ---------------------------------------------------------------- roadmap
# The roadmap is a projection over the milestone graph, in derived order. Nothing here is
# authored: the sequence comes from the dependency ranks, the status from the issues and the
# gate, and "now" and "next" from the first unblocked milestones in that sequence.
mj_plan_roadmap() {
  local id st ver ttl now="" nxt="" blk
  if [ "$MJ_JSON" = 1 ]; then
    awk -F'\t' '
      function esc(x) { gsub(/\\/, "\\\\", x); gsub(/"/, "\\\"", x); return x }
      function lst(x,   n, a, i, o) {
        if (x == "") return "[]"
        n = split(x, a, ","); o = ""
        for (i = 1; i <= n; i++) o = o (i > 1 ? "," : "") "\"" esc(a[i]) "\""
        return "[" o "]"
      }
      $1 == "M" { r[++n] = $16 + 0 "\t" $4 + 0 "\t" $0 }
      END {
        for (i = 1; i < n; i++) for (j = i + 1; j <= n; j++) {
          split(r[i], x, "\t"); split(r[j], y, "\t")
          if (x[1] > y[1] || (x[1] == y[1] && x[2] > y[2])) { t = r[i]; r[i] = r[j]; r[j] = t }
        }
        printf "{\"schema\":1,\"milestones\":["
        for (i = 1; i <= n; i++) {
          sub(/^[^\t]*\t[^\t]*\t/, "", r[i]); split(r[i], f, "\t")
          printf "%s{\"id\":\"%s\",\"version\":\"%s\",\"title\":\"%s\",\"status\":\"%s\",\"rank\":%d,\"order\":%d,\"issues\":{\"total\":%d,\"done\":%d,\"ready\":%d,\"blocked\":%d,\"active\":%d,\"verify\":%d,\"cancelled\":%d},\"depends_on\":%s,\"blocked_by\":%s,\"dependents\":%s,\"claims\":%s}", \
            (i > 1 ? "," : ""), esc(f[2]), esc(f[15]), esc(f[6]), esc(f[3]), f[16], f[4], \
            f[8], f[9], f[10], f[11], f[12], f[13], f[14], \
            lst(f[17]), lst(f[18]), lst(f[19]), lst(f[20])
        }
        printf "]}\n"
      }' "$MJ_PJ/model.tsv"
    return 0
  fi
  printf 'VERSION  STATUS      MILESTONE                     TITLE\n'
  for id in $(mj_pj_roadmap); do
    st="$(mj_pj_m_status "$id")"; ver="$(mj_pj_m_version "$id")"; ttl="$(mj_pj_m_title "$id")"
    printf '%-8s %-11s %-29s %s\n' "${ver:--}" "$st" "$id" "$ttl"
  done
  for id in $(mj_pj_roadmap); do
    st="$(mj_pj_m_status "$id")"
    case "$st" in DONE|CANCELLED|SUPERSEDED) continue ;; esac
    if [ -z "$(mj_pj_m_blocked "$id")" ] && [ -z "$now" ]; then now="$id"; continue; fi
    [ -z "$nxt" ] && nxt="$id"
  done
  printf '\n'
  [ -n "$now" ] && printf 'now:  %s — %s\n' "$now" "$(mj_pj_m_title "$now")"
  if [ -n "$nxt" ]; then
    blk="$(mj_pj_m_blocked "$nxt")"
    printf 'next: %s — %s%s\n' "$nxt" "$(mj_pj_m_title "$nxt")" \
      "$([ -n "$blk" ] && printf ' (blocked by %s)' "$blk")"
  fi
  return 0
}

# ---------------------------------------------------------------- validate
mj_plan_validate() {
  local unk line lvl code subj msg fails=0
  unk="$(mj_project_unknown_keys || true)"
  if [ -n "$unk" ]; then
    printf '%s\n' "$unk" | while read -r rec keys; do
      mj_fail schema "$rec" "unknown keys: $keys" "majordomus plan validate"
    done
    fails=1
  fi
  mj_pj_findings | while IFS="$(printf '\t')" read -r _ lvl code subj msg; do
    case "$lvl" in
      FAIL) mj_fail "$code" "$subj" "$msg" "majordomus plan show $subj" ;;
      *)    mj_warn "$code" "$subj" "$msg" "majordomus plan show $subj" ;;
    esac
  done
  local nf nw ni nm
  nf="$(mj_pj_fail_count)"; nw="$(mj_pj_warn_count)"
  ni="$(mj_pj_issue_ids | wc -l | tr -d ' ')"; nm="$(mj_pj_milestone_ids | wc -l | tr -d ' ')"
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"milestones":%s,"issues":%s,"failures":%s,"warnings":%s,"schema_errors":%s}\n' \
      "$nm" "$ni" "$nf" "$nw" "$fails"
  else
    printf 'plan validate: %s milestone(s), %s issue(s), %s failure(s), %s warning(s)\n' "$nm" "$ni" "$nf" "$nw"
  fi
  { [ "$nf" = 0 ] && [ "$fails" = 0 ]; } || exit "$MJ_EX_CONTRACT"
}

# ---------------------------------------------------------------- status
mj_plan_status() {
  local only="${1:-}" id row active nxt
  active="$(mj_pj_active)"
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"active_milestone":"%s","next_ready":"%s","milestones":[' "$active" "$(mj_pj_next_ready "$active")"
    local first=1
    mj_pj_milestone_ids | while read -r id; do
      [ -n "$only" ] && [ "$id" != "$only" ] && continue
      [ "$first" = 1 ] || printf ','; first=0
      row="$(mj_pj_row M "$id")"
      printf '{"id":"%s","status":"%s","title":"%s","total":%s,"done":%s,"ready":%s,"blocked":%s,"active":%s,"verify":%s,"cancelled":%s}' \
        "$id" "$(printf '%s' "$row" | cut -f3)" "$(mj_json_esc "$(printf '%s' "$row" | cut -f6)")" \
        "$(printf '%s' "$row" | cut -f8)" "$(printf '%s' "$row" | cut -f9)" "$(printf '%s' "$row" | cut -f10)" \
        "$(printf '%s' "$row" | cut -f11)" "$(printf '%s' "$row" | cut -f12)" "$(printf '%s' "$row" | cut -f13)" \
        "$(printf '%s' "$row" | cut -f14)"
    done
    printf ']}\n'
    return 0
  fi
  printf '%-6s %-8s %-5s %s\n' ID STATUS DONE TITLE
  mj_pj_milestone_ids | while read -r id; do
    [ -n "$only" ] && [ "$id" != "$only" ] && continue
    row="$(mj_pj_row M "$id")"
    printf '%-6s %-8s %2s/%-2s %s\n' "$id" "$(printf '%s' "$row" | cut -f3)" \
      "$(printf '%s' "$row" | cut -f9)" "$(printf '%s' "$row" | cut -f8)" "$(printf '%s' "$row" | cut -f6)"
  done
  printf '\nactive milestone: %s\n' "${active:-none}"
  nxt="$(mj_pj_next_ready "$active")"
  if [ -n "$nxt" ]; then printf 'next ready issue: %s — %s\n' "$nxt" "$(mj_pj_i_title "$nxt")"
  else printf 'next ready issue: none — run: majordomus plan blocked\n'; fi
}

# ---------------------------------------------------------------- list / states
mj_plan_list() {
  local only="${1:-}" id row
  if [ "$MJ_JSON" = 1 ]; then mj_plan_issues_json "$only"; return 0; fi
  printf '%-7s %-9s %-4s %-6s %s\n' ID STATUS WAVE MSTONE TITLE
  mj_pj_issue_ids | while read -r id; do
    row="$(mj_pj_row I "$id")"
    [ -n "$only" ] && [ "$(printf '%s' "$row" | cut -f3)" != "$only" ] && continue
    printf '%-7s %-9s %-4s %-6s %s\n' "$id" "$(printf '%s' "$row" | cut -f4)" \
      "$(printf '%s' "$row" | cut -f5)" "$(printf '%s' "$row" | cut -f3)" "$(printf '%s' "$row" | cut -f9)"
  done
}
mj_plan_issues_json() {
  local only="${1:-}" first=1 id row
  printf '{"issues":['
  mj_pj_issue_ids | while read -r id; do
    row="$(mj_pj_row I "$id")"
    [ -n "$only" ] && [ "$(printf '%s' "$row" | cut -f3)" != "$only" ] && continue
    [ "$first" = 1 ] || printf ','; first=0
    printf '{"id":"%s","milestone":"%s","status":"%s","wave":%s,"priority":"%s","title":"%s","depends_on":"%s","blocked_by":"%s"}' \
      "$id" "$(printf '%s' "$row" | cut -f3)" "$(printf '%s' "$row" | cut -f4)" "$(printf '%s' "$row" | cut -f5)" \
      "$(printf '%s' "$row" | cut -f6)" "$(mj_json_esc "$(printf '%s' "$row" | cut -f9)")" \
      "$(printf '%s' "$row" | cut -f11)" "$(printf '%s' "$row" | cut -f12)"
  done
  printf ']}\n'
}
mj_plan_state_list() {
  local want="$1" only="${2:-}" id n=0
  for id in $(mj_pj_in_state "$want" "$only"); do
    n=$((n + 1))
    if [ "$want" = BLOCKED ]; then
      printf '%-7s %-4s %s\n    waiting on: %s\n' "$id" "w$(mj_pj_i_wave "$id")" "$(mj_pj_i_title "$id")" "$(mj_pj_i_blocked "$id")"
    else
      printf '%-7s %-4s %s\n    scope: %s\n' "$id" "w$(mj_pj_i_wave "$id")" "$(mj_pj_i_title "$id")" "$(mj_pj_i_scope "$id")"
    fi
  done
  [ "$n" = 0 ] && printf 'no %s issue%s\n' "$(printf '%s' "$want" | tr 'A-Z' 'a-z')" "${only:+ in $only}"
  return 0
}

# ---------------------------------------------------------------- waves
# Two issues share a wave when the graph allows them to run at the same time. That is a
# necessary condition, not a sufficient one: overlapping scope serialises them, and the
# overlap is reported here rather than left for two workers to discover in a conflict.
mj_plan_waves() {
  local only="${1:-}" w line id st keep
  mj_pj_rows W | while IFS="$(printf '\t')" read -r _ w line; do
    keep=""
    for id in $line; do
      [ -n "$only" ] && [ "$(mj_pj_col I "$id" 3)" != "$only" ] && continue
      keep="$keep $id"
    done
    [ -n "$keep" ] || continue
    printf 'Wave %s\n' "$w"
    for id in $keep; do
      st="$(mj_pj_i_status "$id")"
      printf '  %-7s %-9s %s\n' "$id" "$st" "$(mj_pj_i_title "$id")"
    done
  done
  local conflicts
  conflicts="$(mj_pj_findings | awk -F'\t' '$3=="scope_conflict"')"
  if [ -n "$conflicts" ]; then
    printf '\nserialised by scope overlap:\n'
    printf '%s\n' "$conflicts" | while IFS="$(printf '\t')" read -r _ _ _ subj msg; do
      printf '  %s %s\n' "$subj" "$msg"
    done
  fi
}

mj_plan_next() {
  local nxt; nxt="$(mj_pj_next_ready "${1:-$(mj_pj_active)}")"
  [ -n "$nxt" ] || { printf 'no ready issue; run: majordomus plan blocked\n'; return 0; }
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"id":"%s","title":"%s","milestone":"%s","wave":%s}\n' "$nxt" \
      "$(mj_json_esc "$(mj_pj_i_title "$nxt")")" "$(mj_pj_col I "$nxt" 3)" "$(mj_pj_i_wave "$nxt")"
    return 0
  fi
  printf '%s — %s\n' "$nxt" "$(mj_pj_i_title "$nxt")"
  printf 'objective: %s\n' "$(mj_pj_get "$nxt" objective)"
  printf 'scope:     %s\n' "$(mj_pj_i_scope "$nxt")"
  printf 'profile:   %s\n' "$(mj_pj_get "$nxt" profile)"
  printf '\nnext: majordomus plan start %s\n' "$nxt"
}

# ---------------------------------------------------------------- show
mj_plan_show() {
  local id="$1" f
  [ -n "$id" ] || mj_die "$MJ_EX_USAGE" "plan show needs an id (see: majordomus plan list)"
  f="$(mj_pj_flat "$id")"
  [ -f "$f" ] || mj_die "$MJ_EX_MISSING" "no milestone or issue '$id'"
  mj_plan_body "$id"
}

# ---------------------------------------------------------------- projection body
# One provider-neutral Markdown rendering of a record. The GitHub adapter posts this,
# the site renders the same fields, `plan show` prints it. There is no second copy.
mj_plan_body() {
  local id="$1" f k
  [ -n "$id" ] || mj_die "$MJ_EX_USAGE" "plan body needs an id"
  f="$(mj_pj_flat "$id")"
  [ -f "$f" ] || mj_die "$MJ_EX_MISSING" "no milestone or issue '$id'"
  if mj_pj_is_milestone "$id"; then mj_plan_body_milestone "$id"; else mj_plan_body_issue "$id"; fi
}

mj_plan_sec() { # heading, flat key (scalar)
  local v; v="$(mj_pj_get "$2" "$3")"
  [ -n "$v" ] || return 0
  printf '## %s\n\n%s\n\n' "$1" "$v"
}
mj_plan_seclist() { # heading, id, list key, bullet prefix, item suffix
  local v line; v="$(mj_pj_list "$2" "$3")"
  [ -n "$v" ] || return 0
  printf '## %s\n\n' "$1"
  printf '%s\n' "$v" | while IFS= read -r line; do printf '%s%s%s\n' "${4:-- }" "$line" "${5:-}"; done
  printf '\n'
}

mj_plan_body_issue() {
  local id="$1" st dep d
  st="$(mj_pj_i_status "$id")"
  printf '# %s — %s\n\n' "$id" "$(mj_pj_i_title "$id")"
  printf '| field | value |\n|---|---|\n'
  printf '| status | **%s** (derived) |\n' "$st"
  printf '| milestone | %s |\n' "$(mj_pj_col I "$id" 3)"
  printf '| wave | %s |\n' "$(mj_pj_i_wave "$id")"
  printf '| priority | %s |\n' "$(mj_pj_get "$id" priority)"
  printf '| profile | %s |\n' "$(mj_pj_get "$id" profile)"
  printf '| parallel safe | %s |\n' "$(mj_pj_col I "$id" 8)"
  printf '| canonical | `.majordomus/project/issues/%s.yaml` |\n\n' "$id"
  mj_plan_sec Objective "$id" objective
  mj_plan_sec Why "$id" why
  mj_plan_sec 'Current State' "$id" current_state
  mj_plan_sec 'Desired State' "$id" desired_state
  mj_plan_seclist Scope "$id" scope '- `' '`'
  mj_plan_seclist 'Out of Scope' "$id" non_scope
  printf '## Dependencies\n\n'
  dep="$(mj_pj_i_deps "$id")"
  if [ -z "$dep" ]; then printf 'None. This issue is a root of the graph.\n\n'
  else
    for d in $(printf '%s' "$dep" | tr ',' ' '); do
      printf -- '- %s %s — %s\n' "$d" "$(mj_pj_i_status "$d")" "$(mj_pj_i_title "$d")"
    done
    printf '\n'
  fi
  mj_plan_seclist 'Acceptance Criteria' "$id" acceptance_criteria '- [ ] '
  mj_plan_seclist Validation "$id" validation '- `' '`'
  mj_plan_seclist 'Evidence Required' "$id" evidence_required
  mj_plan_evidence_table "$id"
  printf '## Execution Notes\n\nRisk: %s\n\nThis body is generated from the canonical record by `majordomus plan body %s`.\nEdit the YAML, not this text.\n\n' \
    "$(mj_pj_get "$id" risk)" "$id"
  printf '## Completion Report\n\n%s\n' "$(mj_pj_get "$id" completion || true)"
}

mj_plan_body_milestone() {
  local id="$1" row i
  row="$(mj_pj_row M "$id")"
  printf '# %s — %s\n\n' "$id" "$(mj_pj_m_title "$id")"
  printf '| field | value |\n|---|---|\n'
  printf '| status | **%s** (derived) |\n' "$(printf '%s' "$row" | cut -f3)"
  printf '| issues | %s done of %s |\n' "$(printf '%s' "$row" | cut -f9)" "$(printf '%s' "$row" | cut -f8)"
  printf '| ready / blocked / active / verify | %s / %s / %s / %s |\n' \
    "$(printf '%s' "$row" | cut -f10)" "$(printf '%s' "$row" | cut -f11)" \
    "$(printf '%s' "$row" | cut -f12)" "$(printf '%s' "$row" | cut -f13)"
  printf '| canonical | `.majordomus/project/milestones/%s.yaml` |\n\n' "$id"
  mj_plan_sec Problem "$id" problem
  mj_plan_sec Outcome "$id" outcome
  mj_plan_sec 'Current State' "$id" current_state
  mj_plan_sec 'Desired State' "$id" desired_state
  mj_plan_seclist Scope "$id" scope
  mj_plan_seclist 'Out of Scope' "$id" non_scope
  mj_plan_seclist 'Acceptance Criteria' "$id" acceptance_criteria '- [ ] '
  mj_plan_seclist Validation "$id" validation '- `' '`'
  mj_plan_seclist 'Evidence Required' "$id" evidence_required
  mj_plan_seclist Risks "$id" risks
  mj_plan_evidence_table "$id"
  printf '## Issue DAG\n\n'
  printf '| issue | status | wave | depends on | title |\n|---|---|---|---|---|\n'
  for i in $(mj_pj_of_milestone "$id"); do
    printf '| %s | %s | %s | %s | %s |\n' "$i" "$(mj_pj_i_status "$i")" "$(mj_pj_i_wave "$i")" \
      "$(mj_pj_i_deps "$i" | sed 's/,/, /g')" "$(mj_pj_i_title "$i")"
  done
  printf '\n```mermaid\n%s```\n' "$(mj_project_mermaid "$id")"
}

mj_plan_evidence_table() {
  local id="$1" i=0 c
  c="$(mj_pj_get "$id" evidence.0.covers)"
  [ -n "$c" ] || return 0
  printf '## Evidence\n\n| covers | type | command | result |\n|---|---|---|---|\n'
  while [ -n "$(mj_pj_get "$id" "evidence.$i.covers")" ]; do
    printf '| %s | %s | `%s` | %s |\n' "$(mj_pj_get "$id" "evidence.$i.covers")" \
      "$(mj_pj_get "$id" "evidence.$i.type")" "$(mj_pj_get "$id" "evidence.$i.command")" \
      "$(mj_pj_get "$id" "evidence.$i.result")"
    i=$((i + 1))
  done
  printf '\n'
}

# ---------------------------------------------------------------- transitions
# A transition writes one field. It refuses when the graph says the move is illegal, so
# an issue cannot be started before its dependencies or completed without its evidence.
mj_plan_transition() {
  local id="$1" what="$2" st bb f
  [ -n "$id" ] || mj_die "$MJ_EX_USAGE" "plan $what needs an issue id"
  f="$MJ_DIR/project/issues/$id.yaml"
  [ -f "$f" ] || mj_die "$MJ_EX_MISSING" "no issue '$id'"
  st="$(mj_pj_i_status "$id")"; bb="$(mj_pj_i_blocked "$id")"
  case "$what" in
    start)
      [ "$st" = READY ] || mj_die "$MJ_EX_REFUSED" "$id is $st, not READY${bb:+ (waiting on $bb)}"
      mj_pj_set_field "$id" started_at "$(mj_now)"
      mj_pj_set_field "$id" updated_at "$(mj_now)"
      mj_ledger_append plan_start "\"issue\":\"$id\""
      printf 'plan: %s ACTIVE\n' "$id" ;;
    verify)
      case "$st" in ACTIVE|VERIFY) ;; *) mj_die "$MJ_EX_REFUSED" "$id is $st; only an ACTIVE issue can move to VERIFY" ;; esac
      mj_pj_set_field "$id" verified_at "$(mj_now)"
      mj_pj_set_field "$id" updated_at "$(mj_now)"
      mj_ledger_append plan_verify "\"issue\":\"$id\""
      printf 'plan: %s VERIFY\n' "$id" ;;
    done)
      [ -n "$bb" ] && mj_die "$MJ_EX_REFUSED" "$id cannot be DONE while $bb is not DONE"
      local need missing=""
      for need in $(mj_pj_list "$id" evidence_required); do
        mj_plan_has_evidence "$id" "$need" || missing="$missing $need"
      done
      [ -n "$missing" ] && mj_die "$MJ_EX_CONTRACT" "$id has no evidence for:$missing (run: majordomus plan evidence $id --covers <token> ...)"
      mj_pj_set_field "$id" completed_at "$(mj_now)"
      mj_pj_set_field "$id" updated_at "$(mj_now)"
      mj_ledger_append plan_done "\"issue\":\"$id\""
      printf 'plan: %s DONE\n' "$id" ;;
  esac
  mj_project_unload
  mj_project_load >/dev/null 2>&1 || true
  local nxt; nxt="$(mj_pj_next_ready)"
  printf 'next ready issue: %s\n' "${nxt:-none}"
}

mj_plan_has_evidence() {
  local id="$1" want="$2" i=0 c
  while :; do
    c="$(mj_pj_get "$id" "evidence.$i.covers")"
    [ -n "$c" ] || return 1
    [ "$c" = "$want" ] && return 0
    i=$((i + 1))
  done
}

# Evidence is appended to the issue's own file. There is no separate ledger of evidence:
# the contract and the proof that it was met live in one record, so neither can be read
# without the other.
mj_plan_evidence() {
  local id="$1" covers="$2" etype="$3" ecmd="$4" eres="$5" eart="$6" f tmp
  [ -n "$id" ] || mj_die "$MJ_EX_USAGE" "plan evidence needs an issue or milestone id"
  [ -n "$covers" ] || mj_die "$MJ_EX_USAGE" "plan evidence needs --covers <token>"
  [ -n "$etype" ] || mj_die "$MJ_EX_USAGE" "plan evidence needs --type <test|build|ci|artifact|manual>"
  case "$etype" in test|build|ci|artifact|manual) ;; *) mj_die "$MJ_EX_USAGE" "unknown evidence type '$etype'" ;; esac
  [ -n "$ecmd" ] || [ -n "$eart" ] || mj_die "$MJ_EX_USAGE" "plan evidence needs --command or --artifact; narrative is not evidence"
  # A milestone is gated on evidence the same way an issue is — it reaches DONE only when
  # its own acceptance is proved, not when its issues run out — so the command writes to
  # either record rather than making milestone acceptance the one thing done by hand.
  f="$MJ_DIR/project/issues/$id.yaml"
  [ -f "$f" ] || f="$MJ_DIR/project/milestones/$id.yaml"
  [ -f "$f" ] || mj_die "$MJ_EX_MISSING" "no issue or milestone '$id'"
  mj_pj_list "$id" evidence_required | grep -qx -- "$covers" \
    || mj_die "$MJ_EX_REFUSED" "$id does not require evidence '$covers' (declared: $(mj_pj_list "$id" evidence_required | paste -sd, -))"
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.ev.XXXXXX")"
  {
    if grep -qE '^evidence:' "$f"; then cat "$f"
    else cat "$f"; printf 'evidence:\n'; fi
    printf -- '  - covers: %s\n    type: %s\n' "$covers" "$etype"
    [ -n "$ecmd" ] && printf '    command: "%s"\n' "$(printf '%s' "$ecmd" | sed 's/"/\\"/g')"
    [ -n "$eres" ] && printf '    result: "%s"\n' "$(printf '%s' "$eres" | sed 's/"/\\"/g')"
    [ -n "$eart" ] && printf '    artifact: "%s"\n' "$(printf '%s' "$eart" | sed 's/"/\\"/g')"
    printf '    commit: %s\n    recorded_at: %s\n' "$(mj_git_head)" "$(mj_now)"
  } > "$tmp"
  cat "$tmp" > "$f"
  rm -f "$tmp"
  mj_pj_set_field "$id" updated_at "$(mj_now)"
  mj_ledger_append plan_evidence "\"issue\":\"$id\",\"covers\":\"$covers\",\"type\":\"$etype\""
  printf 'plan: %s evidence recorded for %s\n' "$id" "$covers"
}

#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_session:-}" ] && return 0 || MJ_LIB_session=1
# session — one execution episode of one worker.
#
# A task is a unit of work and can outlive the worker doing it. A session is the worker's
# sitting: it opens, it may cross several tasks, and it closes. Neither contains the other,
# which is why they are two records rather than one field.
#
# The open session lives in state/session-current.yaml and holds only what is true at the
# open. Nothing appends to it while it is open — not checkpoint, not decision, not plan.
# What the episode produced is derived from the ledger when it closes, because the ledger
# already holds those facts and a second mutable account of them is the thing this record
# exists to avoid being.

mj_session_file() { printf '%s' "$MJ_DIR/state/session-current.yaml"; }
mj_session_dir()  { printf '%s' "$MJ_DIR/state/sessions"; }

# Load the open session into MJ_SES_FLAT. 0 loaded · 1 none · 2 does not parse.
mj_load_session() {
  local f; f="$(mj_session_file)"
  [ -f "$f" ] || return 1
  rm -f "${MJ_SES_FLAT:-}" 2>/dev/null || true
  MJ_SES_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.ses.XXXXXX")"
  mj_yaml_flatten "$f" > "$MJ_SES_FLAT" 2>/dev/null || return 2
  return 0
}
mj_ses() { [ -n "${MJ_SES_FLAT:-}" ] || return 0; mj_yget "$MJ_SES_FLAT" "$1"; }

# An open record whose worktree is not this one belongs to another checkout. state/ is
# tracked, so such a record travels here on a branch; it is reported and never treated as
# this checkout's open session, exactly as a foreign task record is.
mj_session_is_foreign() {
  local w; w="$(mj_ses worktree)"
  [ -n "$w" ] || return 1
  [ "$w" = "$MJ_ROOT" ] && return 1
  return 0
}

mj_cmd_session() {
  local sub="${1:-status}"
  case "$sub" in
    --help|-h|help) mj_session_usage; return 0 ;;
    start|status) shift || true ;;
    *) mj_die "$MJ_EX_USAGE" "session: unknown subcommand '$sub' (see: majordomus session --help)" ;;
  esac
  mj_require_installed
  case "$sub" in
    start)  mj_session_start "$@" ;;
    status) mj_session_status "$@" ;;
  esac
}

mj_session_usage() {
  cat <<H
usage: majordomus session <subcommand> [options]

  start [--owner <who>] [--worker <id>]   open an execution episode in this worktree
  status [--json]                         the open session, or absence          (read-only)

  One open session per worktree. start refuses (15) while one is open.
  A session is not a task: it claims no paths, gates no acceptance, and is optional.
H
}

# ---------------------------------------------------------------- start
mj_session_start() {
  local owner="${USER:-unknown}" worker=""
  while [ $# -gt 0 ]; do case "$1" in
    --owner) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--owner needs a value"; owner="$2"; shift 2 ;;
    --owner=*) owner="${1#--owner=}"; shift ;;
    --worker) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--worker needs a value"; worker="$2"; shift 2 ;;
    --worker=*) worker="${1#--worker=}"; shift ;;
    --help|-h) mj_session_usage; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "session start: unknown option $1" ;;
  esac; done

  local rc=0; mj_load_session || rc=$?
  case "$rc" in
    0) if mj_session_is_foreign; then
         mj_warn session "$(mj_ses session_id)" "the open record here belongs to $(mj_ses worktree); replacing it in this working copy only" "cat .majordomus/state/session-current.yaml"
       else
         mj_die "$MJ_EX_REFUSED" "session $(mj_ses session_id) is open here since $(mj_ses started_at); run majordomus session close first"
       fi ;;
    2) mj_die "$MJ_EX_CONTRACT" "session-current.yaml does not parse; move it aside or repair it (run: majordomus doctor)" ;;
  esac

  local id now f tmp
  id="s-$(mj_now_compact | tr -d 'TZ')-$(mj_rand16 | cut -c1-4)"
  now="$(mj_now)"; f="$(mj_session_file)"; tmp="$f.mj-tmp"
  {
    printf 'session_id: %s\nstarted_at: %s\nowner: "%s"\n' "$id" "$now" "$(printf '%s' "$owner" | sed 's/"/\\"/g')"
    # Absent stays absent. A worker identity nobody supplied is not inferred, because an
    # inferred one is indistinguishable from a recorded one the moment it is written down.
    [ -n "$worker" ] && printf 'worker: "%s"\n' "$(printf '%s' "$worker" | sed 's/"/\\"/g')"
    printf '# computed from git; never authored\nrepository_id: %s\nworktree: %s\nbranch: %s\nstart_head: %s\nstart_working_tree: %s\n' \
      "$(mj_git_repo_id)" "$MJ_ROOT" "$(mj_git_branch)" "$(mj_git_head)" "$(mj_git_dirty)"
  } > "$tmp"
  chmod 600 "$tmp" 2>/dev/null || true
  mv "$tmp" "$f"
  mj_ledger_append session.started "\"session_id\":\"$id\",\"owner\":\"$(mj_json_esc "$owner")\"${worker:+,\"worker\":\"$(mj_json_esc "$worker")\"}"

  printf 'session %s opened at %s (head %s)\n' "$id" "$now" "$(mj_git_head | cut -c1-7)"
  printf 'next: majordomus plan next; majordomus context; majordomus session close when the episode ends\n'
}

# ---------------------------------------------------------------- status
mj_session_status() {
  local a; for a in "$@"; do case "$a" in
    --help|-h) mj_session_usage; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "session status: unknown option $a" ;;
  esac; done

  local rc=0; mj_load_session || rc=$?
  if [ "$rc" = 2 ]; then
    if [ "$MJ_JSON" = 1 ]; then printf '{"schema":1,"open":null,"error":"session-current.yaml does not parse"}\n'
    else mj_fail session "session-current.yaml" "does not parse" "cat .majordomus/state/session-current.yaml"; fi
    exit "$MJ_EX_CONTRACT"
  fi
  if [ "$rc" = 1 ]; then
    # Absence is an answer, not a failure — the same rule the handover resolver follows.
    if [ "$MJ_JSON" = 1 ]; then printf '{"schema":1,"open":null}\n'
    else printf 'No open session in this worktree.\nnext: majordomus session start\n'; fi
    return 0
  fi

  local id started label age
  id="$(mj_ses session_id)"; started="$(mj_ses started_at)"
  label="$(mj_git_label "$(mj_ses start_head)" "$(mj_ses branch)")"
  age="$(mj_age_human "$(mj_age_minutes "$started" || true)")"
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"schema":1,"open":{"session_id":"%s","started_at":"%s","owner":"%s","worker":"%s","worktree":"%s","branch":"%s","start_head":"%s","start_working_tree":"%s","foreign":%s,"label":"%s"}}\n' \
      "$id" "$started" "$(mj_json_esc "$(mj_ses owner)")" "$(mj_json_esc "$(mj_ses worker)")" \
      "$(mj_json_esc "$(mj_ses worktree)")" "$(mj_json_esc "$(mj_ses branch)")" \
      "$(mj_ses start_head)" "$(mj_ses start_working_tree)" \
      "$(mj_session_is_foreign && printf true || printf false)" "$label"
    return 0
  fi
  printf 'Session:    %s\n' "$id"
  printf 'Opened:     %s (%s)\n' "$started" "$age"
  printf 'Owner:      %s\n' "$(mj_ses owner)"
  [ -n "$(mj_ses worker)" ] && printf 'Worker:     %s\n' "$(mj_ses worker)"
  printf 'Branch:     %s\n' "$(mj_ses branch)"
  printf 'Start head: %s (%s)\n' "$(mj_ses start_head | cut -c1-7)" "$label"
  if mj_session_is_foreign; then
    mj_info session "$id" "belongs to $(mj_ses worktree), not this checkout; nothing here is about it" "cat .majordomus/state/session-current.yaml"
  fi
  case "$label" in
    diverged|different_context)
      printf 'WARNING     the commit this session opened at is not in this history; trust git, not this record\n' ;;
  esac
}

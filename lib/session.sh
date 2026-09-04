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

# shellcheck source=project.sh
. "$MJ_LIB_DIR/project.sh"

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
    start|status|close) shift || true ;;
    *) mj_die "$MJ_EX_USAGE" "session: unknown subcommand '$sub' (see: majordomus session --help)" ;;
  esac
  mj_require_installed
  case "$sub" in
    start)  mj_session_start "$@" ;;
    status) mj_session_status "$@" ;;
    close)  mj_session_close "$@" ;;
  esac
}

mj_session_usage() {
  cat <<H
usage: majordomus session <subcommand> [options]

  start [--owner <who>] [--worker <id>]   open an execution episode in this worktree
  status [--json]                         the open session, or absence          (read-only)
  close [--outcome closed|interrupted]    close it into an immutable record; summary on stdin

  One open session per worktree. start refuses (15) while one is open.
  A session is not a task: it claims no paths, gates no acceptance, and is optional.

  close derives what the episode produced from the ledger. Nothing writes into an open
  session while it is open, so no other command pays for it and there is no second
  mutable account of events the ledger already holds.
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
  mj_ledger_append session.started "\"owner\":\"$(mj_json_esc "$owner")\"${worker:+,\"worker\":\"$(mj_json_esc "$worker")\"}"

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

# ---------------------------------------------------------------- close
# The whole point of the session object is here. Everything the episode produced is
# derived from the ledger at this moment; nothing accumulated it while the session was
# open. The ledger is append-only, written only by Majordomus, ordered by the order the
# commands ran, and already validated by ledger_integrity, which is what makes it a safe
# thing to derive from. Having every command additionally append to a session file would
# put a write on the hot path of commands that today append one line, and would create a
# second mutable account of facts the ledger already holds.
mj_session_close() {
  local outcome=closed
  while [ $# -gt 0 ]; do case "$1" in
    --outcome) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--outcome needs a value"; outcome="$2"; shift 2 ;;
    --outcome=*) outcome="${1#--outcome=}"; shift ;;
    --help|-h) mj_session_usage; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "session close: unknown option $1" ;;
  esac; done
  # Two values, both self-reported and neither verified — the record says so. `closed` is a
  # worker ending an episode deliberately; `interrupted` tells the next reader that the
  # episode was cut short and its records may be incomplete, which is the one thing about
  # an ended session that changes what somebody does next.
  case "$outcome" in closed|interrupted) ;;
    *) mj_die "$MJ_EX_USAGE" "session close: --outcome must be closed or interrupted" ;;
  esac

  local rc=0; mj_load_session || rc=$?
  case "$rc" in
    1) mj_die "$MJ_EX_MISSING" "no open session in this worktree (run: majordomus session start)" ;;
    2) mj_die "$MJ_EX_CONTRACT" "session-current.yaml does not parse; move it aside or repair it (run: majordomus doctor)" ;;
  esac
  mj_session_is_foreign && mj_die "$MJ_EX_REFUSED" \
    "the open record here belongs to $(mj_ses worktree); close it there, not in this checkout"

  local sid started closed_at task profile owner
  sid="$(mj_ses session_id)"; started="$(mj_ses started_at)"; closed_at="$(mj_now)"
  owner="$(mj_ses owner)"
  task=none; profile=none
  if mj_load_current; then task="$(mj_cur id)"; profile="$(mj_cur profile)"; fi

  # The optional authored summary. Identity fields are refused in it for the same reason
  # they are refused in a checkpoint: a body that forges a commit hash looks checkable.
  local body; body="$(mktemp "${TMPDIR:-/tmp}/mj.sb.XXXXXX")"
  if [ ! -t 0 ]; then cat > "$body"; fi
  if [ -s "$body" ] && mj_reject_identity "$body"; then
    rm -f "$body"
    mj_die "$MJ_EX_CONTRACT" "session close: the summary must not contain identity fields; they are computed"
  fi

  local win rec final
  win="$(mktemp "${TMPDIR:-/tmp}/mj.sw.XXXXXX")"
  mj_session_window "$sid" > "$win"
  rec="$(mktemp "${TMPDIR:-/tmp}/mj.sr.XXXXXX")"
  {
    # created_at, head and working_tree describe the close, so the record reads back
    # through the same resolver and the same divergence label as a handover; start_head
    # and start_working_tree describe the open.
    printf -- '---\nschema_version: 1\ncreated_at: %s\ntask_id: %s\nprofile: %s\nowner: "%s"\n' \
      "$closed_at" "$task" "$profile" "$(printf '%s' "$owner" | sed 's/"/\\"/g')"
    printf 'repository_id: %s\nworktree: %s\nbranch: %s\nhead: %s\nworking_tree: %s\nchanged_files:\n' \
      "$(mj_git_repo_id)" "$MJ_ROOT" "$(mj_git_branch)" "$(mj_git_head)" "$(mj_git_dirty)"
    mj_git status --porcelain=v1 2>/dev/null | cut -c4- | sed 's/^.* -> //' | sed 's/^/  - /'
    printf 'session_id: %s\nstarted_at: %s\nclosed_at: %s\noutcome: %s\n' "$sid" "$started" "$closed_at" "$outcome"
    [ -n "$(mj_ses worker)" ] && printf 'worker: "%s"\n' "$(printf '%s' "$(mj_ses worker)" | sed 's/"/\\"/g')"
    printf 'start_head: %s\nstart_working_tree: %s\n' "$(mj_ses start_head)" "$(mj_ses start_working_tree)"
    mj_session_commits "$(mj_ses start_head)"
    mj_session_refs "$win"
    printf -- '---\n'
  } > "$rec"
  if [ -s "$body" ]; then printf '\n' >> "$rec"; cat "$body" >> "$rec"; fi
  rm -f "$body"

  final="$(mj_publish_record "$(mj_session_dir)" "$sid" "$rec")" \
    || { rm -f "$rec" "$win"; mj_die "$MJ_EX_INTERNAL" "could not create a unique session file"; }
  rm -f "$rec" "$win"

  # Appended before the open record is removed, so the closing event carries this
  # session's stamp like every other event of the episode.
  mj_ledger_append session.closed \
    "\"outcome\":\"$outcome\",\"session_path\":\"$(mj_json_esc "${final#"$MJ_ROOT/"}")\""
  rm -f "$(mj_session_file)"
  printf '%s\n' "${final#"$MJ_ROOT/"}"
}

# The ledger lines belonging to this episode: the ones this session stamped, in ledger
# order.
#
# Selection is by the session id the writer stamped on each line, not by a time range. The
# first real run of this code proved why. The ledger is one file per repository; a window
# of "everything after this session opened" collected another worker's tasks, checkpoints
# and handovers, because two sessions were writing to one repository at once and nothing
# in a timestamp can tell them apart. Which episode wrote an event is a fact the machine
# knows at write time, so it is recorded then rather than guessed at afterwards.
#
# A line carrying no session id belongs to no episode. That is the correct answer, not a
# gap: sessions are optional, and work done outside one is attributed to nobody rather
# than to whoever happened to have a session open nearby.
#
# Ledger order is preserved, which is what makes two events inside one second need no
# tiebreak: the file is append-only and written in the order the commands ran.
mj_session_window() {
  local sid="$1" led
  led="$MJ_DIR/state/ledger.jsonl"
  [ -f "$led" ] || return 0
  awk -v s="$sid" '{ v = $0
      if (index(v, "\"session\":\"") == 0) next
      sub(/^.*"session":"/, "", v); sub(/".*$/, "", v)
      if (v == s) print }' "$led"
}

# Commits between the opening commit and this one, shortest form. When the opening commit
# is not an ancestor of the current one — a rebase during the episode — the list would be
# computed across a history that no longer connects, so the single entry `diverged` is
# recorded instead. That is the vocabulary a record's read-time label already uses.
mj_session_commits() {
  local base="$1" tmp
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.sc.XXXXXX")"
  : > "$tmp"
  if [ -n "$base" ] && [ "$base" != NONE ]; then
    if mj_git merge-base --is-ancestor "$base" HEAD 2>/dev/null; then
      mj_git rev-list --reverse "$base..HEAD" 2>/dev/null | cut -c1-7 > "$tmp"
    else
      printf 'diverged\n' > "$tmp"
    fi
  fi
  # An episode that committed nothing writes an empty list, not a bare key. Absent and
  # empty are different facts everywhere else in this record and they are here too.
  mj_session_emit_list commits "$tmp"
  rm -f "$tmp"
  return 0
}

# The reference lists, derived from the window. Each kind is referenced by the identity it
# actually has; nothing invents one. Decisions and questions are referenced by their text
# because their stores have no id field, and that weakness is recorded in SCHEMAS.md rather
# than hidden — an edited decision title becomes a dangling reference that validation
# reports.
mj_session_refs() {
  local win="$1" tmp
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.sf.XXXXXX")"

  mj_session_field "$win" 'task.started|task.checkpoint|task.handed_over|task.finished' task_id > "$tmp"
  mj_session_emit_list tasks "$tmp"

  mj_session_field "$win" 'plan_start|plan_verify|plan_done|plan_evidence' issue > "$tmp"
  mj_session_emit_list issues "$tmp"
  mj_session_milestones_of "$tmp"

  mj_session_field "$win" 'task.checkpoint' checkpoint_path > "$tmp"
  mj_session_emit_list checkpoints "$tmp"

  mj_session_field "$win" 'task.handed_over' handover_path > "$tmp"
  mj_session_emit_list handovers "$tmp"

  mj_session_field "$win" 'decision.recorded' decision > "$tmp"
  mj_session_emit_list decisions "$tmp"

  mj_session_field "$win" 'question.opened|question.resolved' question > "$tmp"
  mj_session_emit_list questions "$tmp"

  awk '/"event":"plan_evidence"/ {
        i = $0; sub(/^.*"issue":"/, "", i); sub(/".*$/, "", i)
        c = $0; sub(/^.*"covers":"/, "", c); sub(/".*$/, "", c)
        if (i != "" && c != "" && !((i ":" c) in seen)) { seen[i ":" c] = 1; print i ":" c } }' "$win" > "$tmp"
  mj_session_emit_list evidence "$tmp"

  rm -f "$tmp"
}

# Values of one JSON field on the window's lines whose event matches a pattern, in ledger
# order, de-duplicated by first appearance. First appearance rather than sorted order: the
# sequence a worker moved through is itself information, and sorting throws it away.
mj_session_field() {
  local win="$1" events="$2" key="$3"
  awk -v ev="$events" -v k="$key" '
    { e = $0; sub(/^.*"event":"/, "", e); sub(/".*$/, "", e)
      if (e !~ "^(" ev ")$") next
      if (index($0, "\"" k "\":\"") == 0) next
      v = $0; sub("^.*\"" k "\":\"", "", v); sub(/".*$/, "", v)
      if (v != "" && !(v in seen)) { seen[v] = 1; print v } }' "$win"
}

# A YAML list, or an explicit empty one. An absent key and an empty list are different
# facts, and a reader should not have to guess which one it is looking at.
mj_session_emit_list() {
  local name="$1" src="$2" v
  if [ ! -s "$src" ]; then printf '%s: []\n' "$name"; return 0; fi
  printf '%s:\n' "$name"
  while IFS= read -r v; do
    [ -n "$v" ] || continue
    printf -- '  - "%s"\n' "$(printf '%s' "$v" | sed 's/\\/\\\\/g; s/"/\\"/g')"
  done < "$src"
}

# The milestones of the issues this episode touched, from the canonical model. Nothing is
# inferred from the shape of an id: the mapping is the one the project model already
# derives, so a session cannot disagree with `plan` about which milestone an issue is in.
mj_session_milestones_of() {
  local src="$1" id m tmp
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.sm.XXXXXX")"
  : > "$tmp"
  if [ -s "$src" ] && mj_project_load 2>/dev/null; then
    while IFS= read -r id; do
      [ -n "$id" ] || continue
      m="$(mj_pj_col I "$id" 3 2>/dev/null)"
      [ -n "$m" ] && printf '%s\n' "$m"
    done < "$src" | awk '!seen[$0]++' > "$tmp"
  fi
  mj_session_emit_list milestones "$tmp"
  rm -f "$tmp"
}

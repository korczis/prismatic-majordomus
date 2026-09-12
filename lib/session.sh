#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_session:-}" ] && return 0 || MJ_LIB_session=1
# session — one execution episode of one worker.
#
# A task is a unit of work and can outlive the worker doing it. A session is the worker's
# sitting: it opens, it may cross several tasks, and it closes. Neither contains the other,
# which is why they are two records rather than one field.
#
# An open session lives in state/sessions-open/<provider session>.yaml and holds only what is
# true at the open. Nothing appends to it while it is open — not checkpoint, not decision,
# not plan. What the episode produced is derived from the ledger when it closes, because the
# ledger already holds those facts and a second mutable account of them is the thing this
# record exists to avoid being.
#
# One episode per provider session, not one per checkout: two windows of the same provider
# open on one worktree are two workers, and folding them into one record stamps one worker's
# ledger lines with the other's id and lets either one's end event close both. state/
# session-current.yaml is the pointer to the episode of this checkout, and lib/common.sh —
# `mj_session_here_file` — is where the whole resolution is written down.

# shellcheck source=project.sh
. "$MJ_LIB_DIR/project.sh"
# the bounded working context this episode freezes when it opens
# shellcheck source=session_context.sh
. "$MJ_LIB_DIR/session_context.sh"
# which of a dirty tree's files are the episode's work product rather than the tool's exhaust
# shellcheck source=changed.sh
. "$MJ_LIB_DIR/changed.sh"

# This process's open episode: the one MJ_SESSION_KEY names, else the one the provider
# session in the environment names when it is open here, else the one the pointer names.
mj_session_file() { mj_session_here_file; }
mj_session_dir()  { printf '%s' "$MJ_STATE_DIR/sessions"; }

# Every episode open in this checkout, as "<key>|<path>", in name order.
mj_session_open_list() {
  local d f
  d="$(mj_session_open_dir)"
  [ -d "$d" ] || return 0
  for f in "$d"/*.yaml; do
    [ -f "$f" ] || continue
    printf '%s|%s\n' "$(basename "$f" .yaml)" "$f"
  done
  return 0
}

# The episodes open here that are not the one being reported. Two windows on one checkout
# is now a supported state rather than a collision, so it is a state the tool says out loud:
# a worker that cannot see the other episode cannot know that the pointer it resolved
# through is a guess between two.
mj_session_others_lines() {
  local mine="$1" line key f sid n=0
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    key="${line%%|*}"; f="${line#*|}"
    sid="$(sed -n 's/^session_id: //p' "$f" | head -n 1)"
    [ -n "$sid" ] && [ "$sid" != "$mine" ] || continue
    [ "$n" = 0 ] && printf 'Also open:  '
    [ "$n" = 0 ] || printf '            '
    printf '%s (%s)\n' "$sid" "$key"
    n=$((n + 1))
  done <<EOF
$(mj_session_open_list)
EOF
  return 0
}
mj_session_others_json() {
  local mine="$1" line key f sid first=1
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    key="${line%%|*}"; f="${line#*|}"
    sid="$(sed -n 's/^session_id: //p' "$f" | head -n 1)"
    [ -n "$sid" ] && [ "$sid" != "$mine" ] || continue
    [ "$first" = 1 ] || printf ','
    first=0
    printf '{"session_id":"%s","key":"%s"}' "$(mj_json_esc "$sid")" "$(mj_json_esc "$key")"
  done <<EOF
$(mj_session_open_list)
EOF
  return 0
}

# Aim the pointer at one episode. Relative, so a state directory that is copied or moved
# still resolves; a symlink and not a copy, because two accounts of one open episode is the
# drift the session record exists to avoid. A filesystem that cannot make one gets a copy
# and a warning, because losing the episode would be the worse failure.
#
# Every write goes to a private name and is moved onto the pointer, and the reason is a
# measured corruption rather than tidiness. This used to `rm -f` the pointer and then create
# it, which is two operations with a window between them:
#
#   worker A   rm -f pointer                                 ln -s -> sessions-open/a.yaml
#   worker B                rm -f pointer   ln -s ... FAILS (A got there first)
#   worker B                                                 cp b.yaml pointer
#
# and that last `cp` follows the symlink A has just created, so B's episode record is
# written *over A's episode file*. Measured on 2026-09-11 against master: ten concurrent
# `session start`s in one checkout left ten open episodes carrying seven to nine distinct
# session ids, reproducibly, every round. The episodes that lost their identity went on
# stamping ledger lines with the winner's id, and a close of either key would publish one
# record claiming to be both — which is the exact fault `111_session_per_provider` was
# written to end, arriving through the pointer instead of through the store.
#
# `mv` is rename(2): it replaces the pointer atomically, it never follows the destination,
# and there is no moment in which the pointer does not exist. The temp name carries the pid
# so two workers never contend for it.
mj_session_point_at() {
  local target="$1" p tmp
  p="$(mj_session_pointer)"
  tmp="$p.mj-aim.$$"
  rm -f "$tmp"
  if ln -s "sessions-open/$(basename "$target")" "$tmp" 2>/dev/null && mv -f "$tmp" "$p" 2>/dev/null; then
    return 0
  fi
  rm -f "$tmp"
  # No symlinks on this filesystem. The copy goes to the private name too and is moved into
  # place for the same reason: a copy written straight onto the pointer is the write that
  # followed somebody else's link.
  cp "$target" "$tmp" 2>/dev/null && mv -f "$tmp" "$p" 2>/dev/null || { rm -f "$tmp"; return 0; }
  mj_err "warning: $(mj_rel "$p") is a copy, not a link; this filesystem has no symlinks"
  return 0
}
# Where a closed record is written and read from: the layer's tracked sessions section when
# the manifest names one, and the checkout-local store when it does not. A closed episode is
# a shared object of the layer (ADR 0014); the open one never is.
mj_session_store() {
  if [ -n "${MJ_SESSIONS_DIR:-}" ]; then printf '%s' "$MJ_SESSIONS_DIR"; else mj_session_dir; fi
}
# The section exists the moment a record needs it, and it carries its contract from the
# distribution: a directory of the layer without one is a finding (ADR 0011), and the
# episode that created it did not cause that.
mj_session_store_ready() {
  local dir; dir="$(mj_session_store)"
  mkdir -p "$dir"
  [ -n "${MJ_SESSIONS_DIR:-}" ] || return 0
  [ -f "$dir/README.md" ] && return 0
  [ -f "$MJ_SKELETON_DIR/ai/repo/sessions/README.md" ] || return 0
  cp "$MJ_SKELETON_DIR/ai/repo/sessions/README.md" "$dir/README.md"
}
# One line naming the work, for a listing: the task's title when the episode had a task.
mj_session_title() {
  local task="$1" sid="$2"
  if [ "$task" != none ] && mj_load_current && [ -n "$(mj_cur title)" ]; then
    printf '%s' "$(mj_cur title)"
  else
    printf 'Session %s on %s' "$sid" "$(mj_git_branch)"
  fi
}

# Load the open session into MJ_SES_FLAT. 0 loaded · 1 none · 2 does not parse.
mj_load_session() {
  local f; f="$(mj_session_file)"
  [ -f "$f" ] || return 1
  rm -f "${MJ_SES_FLAT:-}" 2>/dev/null || true
  MJ_SES_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.ses.XXXXXX")"
  mj_yaml_flatten "$f" > "$MJ_SES_FLAT" 2>/dev/null || return 2
  # the record's shape, from the schema by way of its generated allow-list. A key the
  # schema does not declare is a record written by something else or by an older version,
  # and reading it as if it were ours is how a field silently means nothing. It is a
  # warning and not a refusal: this is local state, and a session record nobody can parse
  # must not stop the command a person actually ran.
  mj_allow_warn session "$MJ_SES_FLAT" "$MJ_ALLOW_DIR/session.txt" "$(mj_rel "$f")"
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
    start|status|close|list|show|latest|context) shift || true ;;
    *) mj_die "$MJ_EX_USAGE" "session: unknown subcommand '$sub' (see: majordomus session --help)" ;;
  esac
  mj_require_installed
  case "$sub" in
    start)  mj_session_start "$@" ;;
    status) mj_session_status "$@" ;;
    close)  mj_session_close "$@" ;;
    list)   mj_session_list "$@" ;;
    show)   mj_session_show "$@" ;;
    latest) mj_session_latest "$@" ;;
    context) mj_session_context_cmd "$@" ;;
  esac
}

mj_session_usage() {
  cat <<H
usage: majordomus session <subcommand> [options]

  start [--owner <who>] [--worker <id>]   open an execution episode in this worktree
  status [--json]                         the open session, or absence          (read-only)
  close [--outcome closed|interrupted]    close it into an immutable record; summary on stdin
         [--provider-session <id>]        close the episode that provider session opened
  list [--all] [--json]                   closed episodes, newest first            (read-only)
  show <session-id> [--json]              one closed record, whole                 (read-only)
  latest [--path] [--json]                the newest that resolves here            (read-only)
  context [<session-id>]                  the working context of an episode        (read-only)

  One open session per provider session, not one per worktree: a second window of the same
  provider gets an episode of its own, and start refuses (15) only while YOUR episode is
  open. --provider-session names it; without one you are the hand-opened episode, of which
  there is at most one. state/session-current.yaml points at the episode of this checkout.
  A session is not a task: it claims no paths, gates no acceptance, and is optional.

  close derives what the episode produced from the ledger. Nothing writes into an open
  session while it is open, so no other command pays for it and there is no second
  mutable account of events the ledger already holds.

  start freezes the context the builder resolved into .ai/local/session-contexts/, and
  close appends what the close knows to the same document. That store is local: it names
  this machine and it is a snapshot of a projection, so neither half of it is shared.

  A provider hook opens and closes the episode where one is wired (majordomus capture
  install). --if-open and --if-none are what make that safe to run on every event: a
  provider that fires twice, resumes or compacts must not open a second episode, and an
  end event with nothing open is not a failure.
H
}

# ---------------------------------------------------------------- start
mj_session_start() {
  local owner="${USER:-unknown}" worker="" if_open=refuse provider="" psession=""
  while [ $# -gt 0 ]; do case "$1" in
    --owner) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--owner needs a value"; owner="$2"; shift 2 ;;
    --owner=*) owner="${1#--owner=}"; shift ;;
    --worker) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--worker needs a value"; worker="$2"; shift 2 ;;
    --worker=*) worker="${1#--worker=}"; shift ;;
    # What an already-open episode means — for THIS provider session, which is the whole
    # point. A person opening one by hand is told that one is open, because that is a
    # mistake worth stopping. A provider hook is not making that mistake: its start event
    # fires again on a resume and on a compaction, and the honest answer to "your episode
    # is already open" is to keep it. A *different* provider session is neither case: it is
    # another worker, and it gets an episode of its own.
    --if-open) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--if-open needs a value"; if_open="$2"; shift 2 ;;
    --if-open=*) if_open="${1#--if-open=}"; shift ;;
    # Who delivered the event. Only something running inside a provider's own hook can
    # name it, which is what makes opened_by: hook a fact rather than a claim; and the
    # provider's own session identity is what ties this episode to the prompt archive,
    # whose records carry the same string.
    --provider) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--provider needs a value"; provider="$2"; shift 2 ;;
    --provider=*) provider="${1#--provider=}"; shift ;;
    --provider-session) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--provider-session needs a value"; psession="$2"; shift 2 ;;
    --provider-session=*) psession="${1#--provider-session=}"; shift ;;
    --help|-h) mj_session_usage; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "session start: unknown option $1" ;;
  esac; done
  case "$if_open" in refuse|keep) ;;
    *) mj_die "$MJ_EX_USAGE" "session start: --if-open must be refuse or keep" ;;
  esac
  # The episode this open is about, from here to the end of the command: the one the
  # provider session names, or the hand-opened one. Everything below — the already-open
  # test, the file written, the ledger stamp — resolves through it.
  [ -n "$psession" ] && MJ_SESSION_KEY="$psession"

  local rc=0; mj_load_session || rc=$?
  case "$rc" in
    0) if mj_session_is_foreign; then
         mj_warn session "$(mj_ses session_id)" "the open record here belongs to $(mj_ses worktree); replacing it in this working copy only" "cat $(mj_rel "$MJ_STATE_DIR")/session-current.yaml"
       elif [ "$if_open" = keep ]; then
         printf 'session %s already open here since %s; kept\n' "$(mj_ses session_id)" "$(mj_ses started_at)"
         return 0
       else
         mj_die "$MJ_EX_REFUSED" "session $(mj_ses session_id) is open here since $(mj_ses started_at); run majordomus session close first"
       fi ;;
    2) mj_die "$MJ_EX_CONTRACT" "$(mj_rel "$(mj_session_file)") does not parse; move it aside or repair it (run: majordomus doctor)" ;;
  esac

  # Where this episode lives: the file named by the provider session that is opening it, or
  # `hand.yaml` when nobody named one. Not `mj_session_file`, which answers the reading
  # question — "which open episode is this process's" — and falls through to the pointer.
  # An episode is written where its own name puts it, and the pointer is then aimed at it.
  local id now f tmp
  id="s-$(mj_now_compact | tr -d 'TZ')-$(mj_rand16 | cut -c1-4)"
  now="$(mj_now)"; f="$(mj_session_key_file "${MJ_SESSION_KEY:-}")"; tmp="$f.mj-tmp"
  mkdir -p "$MJ_STATE_DIR" "$(mj_session_open_dir)"
  {
    printf 'session_id: %s\nstarted_at: %s\nowner: "%s"\n' "$id" "$now" "$(printf '%s' "$owner" | sed 's/"/\\"/g')"
    # Absent stays absent. A worker identity nobody supplied is not inferred, because an
    # inferred one is indistinguishable from a recorded one the moment it is written down.
    [ -n "$worker" ] && printf 'worker: "%s"\n' "$(printf '%s' "$worker" | sed 's/"/\\"/g')"
    # Who the episode belongs to. Only something running inside a provider's own hook can
    # supply these, and until now they reached the working context and stopped there — so
    # `continuity.state` declared a provider it could never report. The record that decides
    # which episode is whose is the record that has to carry the identity it decides by.
    [ -n "$provider" ] && printf 'provider: "%s"\n' "$(printf '%s' "$provider" | sed 's/"/\\"/g')"
    [ -n "$psession" ] && printf 'provider_session: "%s"\n' "$(printf '%s' "$psession" | sed 's/"/\\"/g')"
    printf '# computed from git; never authored\nrepository_id: %s\nworktree: %s\nbranch: %s\nstart_head: %s\nstart_working_tree: %s\n' \
      "$(mj_git_repo_id)" "$MJ_ROOT" "$(mj_git_branch)" "$(mj_git_head)" "$(mj_git_dirty)"
  } > "$tmp"
  chmod 600 "$tmp" 2>/dev/null || true
  mv "$tmp" "$f"
  mj_session_point_at "$f"
  mj_ledger_append session.started "\"owner\":\"$(mj_json_esc "$owner")\"${worker:+,\"worker\":\"$(mj_json_esc "$worker")\"}"

  # The working context is written after the episode exists, and its failure never costs
  # one: a session whose context could not be frozen is still a session, and the store
  # says so in its own log rather than through this command's exit code.
  local ctx; ctx="$(mj_session_context_open "$id" "$([ -n "$provider" ] && printf hook || printf hand)" "$provider" "$psession" "$worker")"

  printf 'session %s opened at %s (head %s)\n' "$id" "$now" "$(mj_git_head | cut -c1-7)"
  [ -n "$ctx" ] && printf 'working context: %s\n' "$ctx"
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
    if [ "$MJ_JSON" = 1 ]; then printf '{"schema":1,"open":null,"error":"%s does not parse"}\n' "$(mj_json_esc "$(mj_rel "$(mj_session_file)")")"
    else mj_fail session "$(mj_rel "$(mj_session_file)")" "does not parse" "cat $(mj_rel "$(mj_session_file)")"; fi
    exit "$MJ_EX_CONTRACT"
  fi
  if [ "$rc" = 1 ]; then
    # Absence is an answer, not a failure — the same rule the handover resolver follows.
    if [ "$MJ_JSON" = 1 ]; then printf '{"schema":1,"open":null,"others":[%s]}\n' "$(mj_session_others_json "")"
    else
      printf 'No open session in this worktree.\n'
      mj_session_others_lines ""
      printf 'next: majordomus session start\n'
    fi
    return 0
  fi

  local id started label age
  id="$(mj_ses session_id)"; started="$(mj_ses started_at)"
  label="$(mj_git_label "$(mj_ses start_head)" "$(mj_ses branch)")"
  age="$(mj_age_human "$(mj_age_minutes "$started" || true)")"
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"schema":1,"open":{"session_id":"%s","started_at":"%s","owner":"%s","worker":"%s","provider":"%s","provider_session":"%s","worktree":"%s","branch":"%s","start_head":"%s","start_working_tree":"%s","foreign":%s,"label":"%s"},"others":[%s]}\n' \
      "$id" "$started" "$(mj_json_esc "$(mj_ses owner)")" "$(mj_json_esc "$(mj_ses worker)")" \
      "$(mj_json_esc "$(mj_ses provider)")" "$(mj_json_esc "$(mj_ses provider_session)")" \
      "$(mj_json_esc "$(mj_ses worktree)")" "$(mj_json_esc "$(mj_ses branch)")" \
      "$(mj_ses start_head)" "$(mj_ses start_working_tree)" \
      "$(mj_session_is_foreign && printf true || printf false)" "$label" \
      "$(mj_session_others_json "$id")"
    return 0
  fi
  printf 'Session:    %s\n' "$id"
  printf 'Opened:     %s (%s)\n' "$started" "$age"
  printf 'Owner:      %s\n' "$(mj_ses owner)"
  [ -n "$(mj_ses worker)" ] && printf 'Worker:     %s\n' "$(mj_ses worker)"
  # Whose episode this is. An episode with no provider was opened by hand and says nothing
  # here, because a blank line claiming a provider is worse than no line.
  [ -n "$(mj_ses provider)" ] && printf 'Provider:   %s%s\n' "$(mj_ses provider)" \
    "$([ -n "$(mj_ses provider_session)" ] && printf ' (session %s)' "$(mj_ses provider_session)")"
  printf 'Branch:     %s\n' "$(mj_ses branch)"
  printf 'Start head: %s (%s)\n' "$(mj_ses start_head | cut -c1-7)" "$label"
  mj_session_others_lines "$id"
  if mj_session_is_foreign; then
    mj_info session "$id" "belongs to $(mj_ses worktree), not this checkout; nothing here is about it" "cat $(mj_rel "$MJ_STATE_DIR")/session-current.yaml"
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
  local outcome=closed if_none=refuse
  while [ $# -gt 0 ]; do case "$1" in
    --outcome) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--outcome needs a value"; outcome="$2"; shift 2 ;;
    --outcome=*) outcome="${1#--outcome=}"; shift ;;
    # What no open episode means. To a person it is a mistake — they meant to close
    # something. To a provider's end event it is the normal case: the episode was closed by
    # hand, or none was ever opened, and refusing there would put a failure in front of
    # somebody leaving the room.
    --if-none) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--if-none needs a value"; if_none="$2"; shift 2 ;;
    --if-none=*) if_none="${1#--if-none=}"; shift ;;
    # Which episode. A provider's end event names its own and closes that or nothing; it
    # never falls through to the pointer, which in a checkout with two windows open would
    # be the other worker's episode.
    --provider-session) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--provider-session needs a value"; MJ_SESSION_KEY="$2"; shift 2 ;;
    --provider-session=*) MJ_SESSION_KEY="${1#--provider-session=}"; shift ;;
    --help|-h) mj_session_usage; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "session close: unknown option $1" ;;
  esac; done
  case "$if_none" in refuse|ignore) ;;
    *) mj_die "$MJ_EX_USAGE" "session close: --if-none must be refuse or ignore" ;;
  esac
  # Two values, both self-reported and neither verified — the record says so. `closed` is a
  # worker ending an episode deliberately; `interrupted` tells the next reader that the
  # episode was cut short and its records may be incomplete, which is the one thing about
  # an ended session that changes what somebody does next.
  case "$outcome" in closed|interrupted) ;;
    *) mj_die "$MJ_EX_USAGE" "session close: --outcome must be closed or interrupted" ;;
  esac

  local rc=0; mj_load_session || rc=$?
  case "$rc" in
    1) [ "$if_none" = ignore ] && return 0
       mj_die "$MJ_EX_MISSING" "no open session in this worktree (run: majordomus session start)" ;;
    2) mj_die "$MJ_EX_CONTRACT" "$(mj_rel "$(mj_session_file)") does not parse; move it aside or repair it (run: majordomus doctor)" ;;
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
    # A shared record carries what the repository can prove and nothing about this machine:
    # the absolute worktree path is a fact about a disk, and the person who ran it is not
    # the repository's business (ADR 0014). Both stay in the ledger, which is local.
    printf -- '---\nschema: session/v1\nkind: session\ncreated_at: %s\ntask_id: %s\nprofile: %s\n' \
      "$closed_at" "$task" "$profile"
    printf 'repository_id: %s\nworktree_id: %s\nbranch: %s\nhead: %s\nworking_tree: %s\nchanged_files:\n' \
      "$(mj_repository_id)" "$(mj_worktree_id)" "$(mj_git_branch)" "$(mj_git_head)" "$(mj_git_dirty)"
    # Classified, not copied. The record at 20260910T103933Z--s-20260909152316-024f named
    # 122 changed files, among them the whole of site/data/generated/, its own three sibling
    # records and its own site projection — a record claiming itself as its own work
    # product. lib/changed.sh filters against the declarations that already say what is
    # derived; what remains is what the episode actually wrote.
    mj_changed_files_block
    printf 'session_id: %s\nstarted_at: %s\nclosed_at: %s\noutcome: %s\n' "$sid" "$started" "$closed_at" "$outcome"
    printf 'title: "%s"\n' "$(printf '%s' "$(mj_session_title "$task" "$sid")" | sed 's/"/\\"/g')"
    [ -n "$(mj_ses worker)" ] && printf 'worker: "%s"\n' "$(printf '%s' "$(mj_ses worker)" | sed 's/"/\\"/g')"
    printf 'start_head: %s\nstart_working_tree: %s\n' "$(mj_ses start_head)" "$(mj_ses start_working_tree)"
    mj_session_commits "$(mj_ses start_head)"
    mj_session_refs "$win"
    printf -- '---\n'
  } > "$rec"
  if [ -s "$body" ]; then printf '\n' >> "$rec"; cat "$body" >> "$rec"; fi
  rm -f "$body"

  mj_session_store_ready
  # One episode, one record. `mj_publish_record` gives each file a unique name, which makes
  # a second close of the same episode a second *file* rather than a collision — so nothing
  # here ever refused it, and the identity contract this section declares was enforced only
  # by the validator, which reads a tree somebody has already committed. Nobody commits a
  # record that would turn the section red, so four records of one episode simply sat
  # untracked: closed at 21:41 and then again at 10:39:02, 10:39:18 and 10:39:31, thirteen
  # and fifteen seconds apart, because a provider's end event can fire more than once.
  #
  # The record is immutable by contract, so the first one written is the record and a later
  # close of the same episode adds nothing a reader could use. It is not an error either —
  # the second end event is the provider's normal behaviour, not a worker's mistake — so
  # this keeps what exists, says so, and still tears down the open record below.
  #
  # Under a lock, because the guard is a check followed by a write and those are not one
  # operation. Two end events arriving together both grepped an empty store, both published,
  # and one episode had two canonical records: measured on 2026-09-11 against master, two
  # concurrent closes of one episode produced two records in two of three trials. The lock
  # is per episode and lives in the local half, which is the scope of the thing it protects:
  # an episode belongs to one checkout, and two checkouts closing must never wait on each
  # other. `mj_lock_take` never refuses — a bounded wait that expires proceeds with the
  # grep alone, which is where this was before, so the worst case is what master already had.
  local existing
  mj_lock_take "$MJ_STATE_DIR/locks/session-close.$sid" || true
  # --exclude: mj_publish_record stages into `.tmp.XXXXXX` inside the store before it hard-
  # links the final name, and a process that dies between the two leaves that file behind
  # (which is what `majordomus recover orphans` is for). Without this, a close would name a
  # temp file as the episode's record — the ledger event, the context close and this
  # command's own output would all point at a path that is about to be removed.
  existing="$(grep -rl --exclude='.tmp.*' "^session_id: $sid\$" "$(mj_session_store)" 2>/dev/null | head -n 1 || true)"
  if [ -n "$existing" ]; then
    # The episode's record is the one that already exists, and every reader downstream —
    # the ledger event, the context close, this command's own output — must name it, not
    # the file that was not written.
    final="$existing"
    rm -f "$rec" "$win"
    printf 'session: %s already has a record at %s; this close adds none\n' \
      "$sid" "$(mj_rel "$existing")" >&2
  else
    final="$(mj_publish_record "$(mj_session_store)" "$sid" "$rec")" \
      || { rm -f "$rec" "$win"; mj_lock_release; mj_die "$MJ_EX_INTERNAL" "could not create a unique session file"; }
    rm -f "$rec" "$win"
  fi
  # The record exists; everything after this is teardown, and holding the lock across it
  # would serialise two closes that are no longer racing for the same thing.
  mj_lock_release

  # What the episode learned, derived on the single path every close takes (ADR 0058):
  # after its record, so that the episode's own record exists before anything names it,
  # and before session.closed, so that the knowledge.derived line of an episode precedes
  # its close in ledger order and the stopped-writer check compares episode ids rather
  # than clocks. Best effort, on purpose: a derivation that fails is said on stderr and
  # reported by the adapter as the typed event, and the episode still closes.
  [ -n "${MJ_POL_FLAT:-}" ] || mj_load_policy 2>/dev/null || true
  if [ "$(mj_pol session.knowledge_on_end)" != false ]; then
    # shellcheck source=knowledge.sh
    . "$MJ_LIB_DIR/knowledge.sh"
    local kout
    if kout="$( (mj_cmd_knowledge derive --episode "$sid") 2>&1 )"; then
      mj_err "session close: knowledge derived: $(printf '%s\n' "$kout" | tail -n 1)"
    else
      mj_err "session close: the knowledge was not derived: $(printf '%s\n' "$kout" | tail -n 1)"
    fi
  else
    mj_err "session close: session.knowledge_on_end is false, so no knowledge is derived"
  fi

  # Appended before the open record is removed, so the closing event carries this
  # session's stamp like every other event of the episode.
  mj_ledger_append session.closed \
    "\"outcome\":\"$outcome\",\"session_path\":\"$(mj_json_esc "${final#"$MJ_ROOT/"}")\""

  # This episode's own file goes, and the pointer goes with it only when it named this
  # episode: closing one window must leave the other window's episode exactly where it was.
  # The two paths can be the same file — a record written before this store existed, or one
  # a person put at the pointer by hand — so both are removed and neither is required.
  local here ptr repoint=0 line
  here="$(mj_session_file)"; ptr="$(mj_session_pointer)"
  if [ -f "$ptr" ] && [ "$(sed -n 's/^session_id: //p' "$ptr" | head -n 1)" = "$sid" ]; then repoint=1; fi
  # By identity and not by the path this process resolved through: a keyless close resolves
  # through the pointer, and removing the pointer would leave the episode itself in the
  # store, open for ever and refusing every later open of the same key.
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    [ "$(sed -n 's/^session_id: //p' "${line#*|}" | head -n 1)" = "$sid" ] && rm -f "${line#*|}"
  done <<EOF
$(mj_session_open_list)
EOF
  rm -f "$here"
  [ "$repoint" = 1 ] && rm -f "$ptr"
  # A pointer left aiming at nothing is the same fact as no pointer, and it is this close
  # that made it one; it is removed here rather than left for a reader to trip over.
  if [ -L "$ptr" ] && [ ! -e "$ptr" ]; then rm -f "$ptr"; repoint=1; fi
  # A pointer this close freed is aimed at the one episode still open here, when there is
  # exactly one. A person whose hand-opened episode outlives a provider's must not be told
  # there is none; with two still open there is nothing to choose between them, and a guess
  # would be worse than the absence `session status` reports instead.
  if [ "$repoint" = 1 ]; then
    local rest n
    rest="$(mj_session_open_list)"
    n="$(printf '%s' "$rest" | grep -c . 2>/dev/null || true)"
    [ "$n" = 1 ] && mj_session_point_at "${rest#*|}"
  fi
  # The working context of the episode learns how it ended. It is appended to, never
  # rewritten, so nothing the worker typed into it between the two events is lost.
  mj_session_context_close "$sid" "$outcome" "${final#"$MJ_ROOT/"}" >/dev/null
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
  led="$MJ_STATE_DIR/ledger.jsonl"
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

# ---------------------------------------------------------------- reading back
# Three read-only views over the closed records. All of them print the record's divergence
# label, and none of them invents a fourth vocabulary for staleness: `exact`, `advanced`,
# `diverged` and `different_context` already mean something everywhere else in this tool.
#
# Ordering is by the recorded timestamp, with ledger position breaking a tie inside one
# second. Filesystem modification time is never read: it does not survive a clone and it is
# not the time the record asserts.

# One sort key per record: "<created_at>|<ledger rank>|<path>", newest first.
mj_session_keys() {
  local dir f fm flat created
  dir="$(mj_session_store)"
  [ -d "$dir" ] || return 0
  for f in "$dir"/*.md; do
    [ -f "$f" ] || continue
    mj_is_context_doc "$f" && continue          # the section's own contract is not a record
    fm="$(mktemp "${TMPDIR:-/tmp}/mj.slf.XXXXXX")"; flat="$(mktemp "${TMPDIR:-/tmp}/mj.slg.XXXXXX")"
    if mj_record_front "$f" > "$fm" 2>/dev/null && mj_yaml_flatten "$fm" > "$flat" 2>/dev/null; then
      created="$(mj_yget "$flat" created_at)"
      if [ -n "$created" ]; then printf '%s|%s|%s\n' "$created" "$(mj_record_rank "$f")" "$f"
      else mj_err "warning: skipped ${f#"$MJ_ROOT/"}: no created_at"; fi
    else
      mj_err "warning: skipped ${f#"$MJ_ROOT/"}: malformed record"
    fi
    rm -f "$fm" "$flat"
  done | LC_ALL=C sort -r
}

# Read one record's front matter into MJ_SREC_* .
# 0 read · 1 does not parse · 2 parsed, but carries a schema version this executable does not read.
MJ_SREC_ID=""; MJ_SREC_CREATED=""; MJ_SREC_STARTED=""; MJ_SREC_OUTCOME=""
MJ_SREC_HEAD=""; MJ_SREC_BRANCH=""; MJ_SREC_WORKER=""; MJ_SREC_REPO=""; MJ_SREC_SCHEMA=""
# The one version of the shared session record this executable reads. Declared once, here,
# because `doctor` and this reader disagreeing about it is how a store comes to be reported
# healthy by one surface and refused by the other.
MJ_SESSION_SCHEMA="session/v1"
# What a record said about its version, for a message: the value, or the fact that there was
# none. `${x:+a}${x:-b}` does not express this — when x is set the second expansion is x, not
# b — and the first draft of these messages read "schema 'session/v2'session/v2".
mj_session_schema_said() {
  if [ -n "${1:-}" ]; then printf "schema '%s'" "$1"; else printf 'no schema'; fi
}
mj_session_read() {
  local f="$1" fm flat rc=0
  fm="$(mktemp "${TMPDIR:-/tmp}/mj.srf.XXXXXX")"; flat="$(mktemp "${TMPDIR:-/tmp}/mj.srg.XXXXXX")"
  MJ_SREC_SCHEMA=""
  if mj_record_front "$f" > "$fm" 2>/dev/null && mj_yaml_flatten "$fm" > "$flat" 2>/dev/null; then
    MJ_SREC_SCHEMA="$(mj_yget "$flat" schema)"
    MJ_SREC_ID="$(mj_yget "$flat" session_id)"; MJ_SREC_CREATED="$(mj_yget "$flat" created_at)"
    MJ_SREC_STARTED="$(mj_yget "$flat" started_at)"; MJ_SREC_OUTCOME="$(mj_yget "$flat" outcome)"
    MJ_SREC_HEAD="$(mj_yget "$flat" head)"; MJ_SREC_BRANCH="$(mj_yget "$flat" branch)"
    MJ_SREC_WORKER="$(mj_yget "$flat" worker)"
    MJ_SREC_REPO="$(mj_yget "$flat" repository_id)"
    # A version this executable does not read is not a record it may report. It parsed, so
    # `rc=1` would be a lie and a silent `continue` in the caller would be worse than either:
    # a record written by a newer Majordomus was listed here as an ordinary closed episode,
    # with its outcome and its branch, and nothing anywhere said that the fields had been
    # read under a contract that no longer describes them. A silent accept of an unknown
    # version is the same defect as a silent skip, pointing the other way.
    # An absent version is refused for the same reason and not a softer one. The contract
    # `doctor` enforces has `schema` as a field every record carries, so a record without it
    # is not an older record this reader is being generous to — it is a record whose contract
    # nobody stated, and reading its fields as though they were this version's is the guess
    # the rest of this file exists not to make.
    [ "$MJ_SREC_SCHEMA" = "$MJ_SESSION_SCHEMA" ] || rc=2
  else rc=1; fi
  rm -f "$fm" "$flat"
  return $rc
}

mj_session_list() {
  local a scope_all=0
  for a in "$@"; do case "$a" in
    --all) scope_all=1 ;;
    --help|-h) mj_session_usage; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "session list: unknown option $a" ;;
  esac; done

  local key f n=0 first=1 label mine rc skipped=0
  # A shared record names the repository by its remote, a local one by its git directory:
  # this is the same repository under either name (ADR 0014).
  mine="$(mj_repository_id)"
  [ "$MJ_JSON" = 1 ] && printf '{"schema":1,"sessions":['
  for key in $(mj_session_keys); do
    f="${key#*|}"; f="${f#*|}"
    rc=0; mj_session_read "$f" || rc=$?
    case "$rc" in
      1) continue ;;
      # Said out loud, on stderr, and skipped. Not on stdout: every caller of `--json`
      # redirects that into a file, and a diagnostic written there is a diagnostic nobody
      # sees inside a document nobody can parse.
      2) mj_err "warning: skipped ${f#"$MJ_ROOT/"}: $(mj_session_schema_said "$MJ_SREC_SCHEMA"), and this executable reads $MJ_SESSION_SCHEMA (run: majordomus doctor)"
         skipped=$((skipped + 1)); continue ;;
    esac
    # Same rule as every other record: this repository, this worktree and branch, then this
    # branch. --all lifts it and says so, because a record from elsewhere is worth seeing
    # when you asked for everything and is never worth being handed silently.
    if [ "$scope_all" = 0 ]; then
      [ "$MJ_SREC_REPO" = "$mine" ] || [ "$MJ_SREC_REPO" = "$(mj_git_repo_id)" ] || continue
      [ "$MJ_SREC_BRANCH" = "$(mj_git_branch)" ] || continue
    fi
    label="$(mj_git_label "$MJ_SREC_HEAD" "$MJ_SREC_BRANCH")"
    if [ "$MJ_JSON" = 1 ]; then
      [ "$first" = 1 ] || printf ','; first=0
      printf '{"session_id":"%s","closed_at":"%s","started_at":"%s","outcome":"%s","branch":"%s","label":"%s","path":"%s"}' \
        "$MJ_SREC_ID" "$MJ_SREC_CREATED" "$MJ_SREC_STARTED" "$MJ_SREC_OUTCOME" \
        "$(mj_json_esc "$MJ_SREC_BRANCH")" "$label" "$(mj_json_esc "${f#"$MJ_ROOT/"}")"
    else
      printf '%s  %-24s %-12s %-18s %s\n' "$MJ_SREC_CREATED" "$MJ_SREC_ID" "$MJ_SREC_OUTCOME" "$label" "${f#"$MJ_ROOT/"}"
    fi
    n=$((n + 1))
  done
  if [ "$MJ_JSON" = 1 ]; then printf ']}\n'; return 0; fi
  [ "$n" = 0 ] && printf 'No closed sessions for this worktree and branch.\n'
  # An empty listing that is empty because every record was refused is a different answer
  # from an empty listing, and a reader who is not told cannot tell them apart.
  [ "$skipped" -gt 0 ] && printf '%s record(s) were skipped: this executable reads %s (run: majordomus doctor)\n' "$skipped" "$MJ_SESSION_SCHEMA"
  return 0
}

mj_session_latest() {
  local path_only=0 a
  for a in "$@"; do case "$a" in
    --path) path_only=1 ;;
    --help|-h) mj_session_usage; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "session latest: unknown option $a" ;;
  esac; done
  # The shared resolver, not a second rule: same repository, same worktree, same branch,
  # then same branch, then nothing. A record from an unrelated worktree is never offered,
  # because borrowed context cannot be recognised as wrong until it has been acted on.
  if ! mj_resolve_latest "$(mj_session_store)" ""; then
    if [ "$MJ_JSON" = 1 ]; then printf '{"schema":1,"latest":null}\n'
    else printf 'No closed session for this worktree and branch.\n'; fi
    return 0
  fi
  [ "$path_only" = 1 ] && { printf '%s\n' "${MJ_RES_PATH#"$MJ_ROOT/"}"; return 0; }
  mj_session_show_file "$MJ_RES_PATH" "$MJ_RES_MATCH"
}

# ---------------------------------------------------------------- context
# Where the working context of an episode is, so that a person who wants to add to it does
# not have to know how the store names its files. Read-only, and it prints a path rather
# than the document: the document is local evidence, and a command that pours it into a
# terminal invites it into somebody's context, which is the one thing the local half of the
# layer forbids.
mj_session_context_cmd() {
  local sid="" a
  for a in "$@"; do case "$a" in
    --help|-h) mj_session_usage; return 0 ;;
    -*) mj_die "$MJ_EX_USAGE" "session context: unknown option $a" ;;
    *) [ -n "$sid" ] && mj_die "$MJ_EX_USAGE" "session context: one session id at a time"; sid="$a" ;;
  esac; done

  if [ -z "$sid" ]; then
    local rc=0; mj_load_session || rc=$?
    [ "$rc" = 2 ] && mj_die "$MJ_EX_CONTRACT" "$(mj_rel "$(mj_session_file)") does not parse; move it aside or repair it (run: majordomus doctor)"
    if [ "$rc" = 1 ]; then
      if [ "$MJ_JSON" = 1 ]; then printf '{"schema":1,"session_id":null,"context":null}\n'
      else printf 'No open session in this worktree.\nnext: majordomus session start\n'; fi
      return 0
    fi
    sid="$(mj_ses session_id)"
  fi

  local out; out="$(mj_session_context_path "$sid")"
  if [ -z "$out" ]; then
    if [ "$MJ_JSON" = 1 ]; then printf '{"schema":1,"session_id":"%s","context":null}\n' "$sid"
    else printf 'No working context for %s under %s.\n' "$sid" "$(mj_rel "$(mj_session_context_dir)")"; fi
    return "$MJ_EX_MISSING"
  fi
  # The document's age in the vocabulary `session show` already uses for a closed record.
  #
  # It rides in the JSON only. The text form of this command is a path and stays one: the
  # command exists so that a person who wants to add to the document does not have to know
  # how the store names its files, callers consume `$(majordomus session context)` as that
  # path, and a second line on standard output would turn every one of them into a caller of
  # a two-line path. The surface a worker reads for the same fact is the context builder,
  # whose GIT section labels the briefing beside the head it is being compared with.
  local fresh label rhead since
  fresh="$(mj_session_context_freshness "$sid")" || fresh="unknown	NONE	0"
  label="${fresh%%	*}"; fresh="${fresh#*	}"
  rhead="${fresh%%	*}"; since="${fresh#*	}"
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"schema":1,"session_id":"%s","context":"%s","label":"%s","recorded_head":"%s","commits_since":%s}\n' \
      "$sid" "$(mj_json_esc "${out#"$MJ_ROOT/"}")" "$label" "$rhead" "$since"
  else
    printf '%s\n' "${out#"$MJ_ROOT/"}"
  fi
}

mj_session_show() {
  local want="" a
  for a in "$@"; do case "$a" in
    --help|-h) mj_session_usage; return 0 ;;
    -*) mj_die "$MJ_EX_USAGE" "session show: unknown option $a" ;;
    *) [ -z "$want" ] || mj_die "$MJ_EX_USAGE" "session show: one session id only"; want="$a" ;;
  esac; done
  [ -n "$want" ] || mj_die "$MJ_EX_USAGE" "session show needs a session id (see: majordomus session list)"

  local key f hit="" unreadable="" rc
  for key in $(mj_session_keys); do
    f="${key#*|}"; f="${f#*|}"
    rc=0; mj_session_read "$f" || rc=$?
    # A record this version cannot read is remembered rather than passed over. Skipping it
    # makes `session show <id>` answer "no closed session" about a record that is sitting in
    # the store under exactly that id — which sends the reader to look for a record that is
    # there, instead of telling them the one thing they need to know about it.
    [ "$rc" = 2 ] && { grep -q "^session_id: $want\$" "$f" 2>/dev/null && unreadable="$f"; continue; }
    [ "$rc" = 0 ] || continue
    [ "$MJ_SREC_ID" = "$want" ] || continue
    hit="$f"; break
  done
  [ -n "$hit" ] || [ -z "$unreadable" ] || mj_session_show_file "$unreadable" ""
  [ -n "$hit" ] || mj_die "$MJ_EX_MISSING" "no closed session '$want' (see: majordomus session list)"
  mj_session_read "$hit" >/dev/null 2>&1 || true
  mj_session_show_file "$hit" ""
}

# Print one record whole, headed by the facts a reader needs before believing any of it.
mj_session_show_file() {
  local f="$1" match="$2" label rc=0
  mj_session_read "$f" || rc=$?
  # Two different refusals, and a reader who is told the wrong one looks in the wrong place:
  # a record that does not parse is damaged here, a record carrying a version this executable
  # does not read is intact and was written by something newer.
  [ "$rc" = 2 ] && mj_die "$MJ_EX_CONTRACT" \
    "${f#"$MJ_ROOT/"} carries $(mj_session_schema_said "$MJ_SREC_SCHEMA"), and this executable reads $MJ_SESSION_SCHEMA (run: majordomus doctor)"
  [ "$rc" = 0 ] || mj_die "$MJ_EX_CONTRACT" "session record does not parse: ${f#"$MJ_ROOT/"}"
  label="$(mj_git_label "$MJ_SREC_HEAD" "$MJ_SREC_BRANCH")"
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"schema":1,"session":{"session_id":"%s","started_at":"%s","closed_at":"%s","outcome":"%s","branch":"%s","label":"%s","match":"%s","path":"%s"}}\n' \
      "$MJ_SREC_ID" "$MJ_SREC_STARTED" "$MJ_SREC_CREATED" "$MJ_SREC_OUTCOME" \
      "$(mj_json_esc "$MJ_SREC_BRANCH")" "$label" "$match" "$(mj_json_esc "${f#"$MJ_ROOT/"}")"
    return 0
  fi
  printf 'Session:   %s\n' "$MJ_SREC_ID"
  printf 'Record:    %s\n' "${f#"$MJ_ROOT/"}"
  [ -n "$match" ] && printf 'Match:     %s\n' "$match"
  printf 'Episode:   %s .. %s\n' "$MJ_SREC_STARTED" "$MJ_SREC_CREATED"
  printf 'Outcome:   %s\n' "$MJ_SREC_OUTCOME"
  [ -n "$MJ_SREC_WORKER" ] && printf 'Worker:    %s\n' "$MJ_SREC_WORKER"
  printf 'Git state: %s (closed at %s)\n' "$label" "$(printf '%s' "$MJ_SREC_HEAD" | cut -c1-7)"
  case "$label" in
    diverged|different_context)
      printf 'WARNING    this record describes a history this checkout no longer has; trust git, not it\n' ;;
  esac
  printf -- '---\n'
  mj_record_front "$f" | sed -n '/^session_id:/,$p'
  printf -- '---\n'
  mj_record_body "$f"
}

# ---------------------------------------------------------------- the doctrine
# majordomus.session-records: a closed episode is a shared object with a closed field set,
# carrying what the repository can prove and nothing about the machine that ran it.
#
# The Rust index refuses an unknown key when it builds; this is the same contract read from
# the shell, so `doctor` answers for the section without a toolchain present.
mj_validate_session_records() {
  local dir f fm flat n=0 bad=0 ids="" id absolute
  dir="$(mj_session_store)"
  [ -n "${MJ_SESSIONS_DIR:-}" ] || return 0      # a layer with no sessions section owes nothing
  [ -d "$dir" ] || return 0
  for f in "$dir"/*.md; do
    [ -f "$f" ] || continue
    mj_is_context_doc "$f" && continue           # the section's own contract is not a record
    n=$((n + 1))
    fm="$(mktemp "${TMPDIR:-/tmp}/mj.svf.XXXXXX")"; flat="$(mktemp "${TMPDIR:-/tmp}/mj.svg.XXXXXX")"
    if ! mj_record_front "$f" > "$fm" 2>/dev/null || ! mj_yaml_flatten "$fm" > "$flat" 2>/dev/null; then
      mj_doctrine_fail session "$(basename "$f")" "front matter does not parse" "head -n 20 $(mj_rel "$f")"
      bad=1; rm -f "$fm" "$flat"; continue
    fi
    rm -f "$fm"
    local unknown; unknown="$(mj_yaml_unknown_keys "$flat" "$MJ_ALLOW_DIR/session-record.txt" || true)"
    [ -n "$unknown" ] && {
      mj_doctrine_fail session "$(basename "$f")" "front-matter key(s) the contract does not have: $(printf '%s' "$unknown" | tr '\n' ' ' | sed 's/ $//')" "head -n 20 $(mj_rel "$f")"
      bad=1
    }
    [ "$(mj_yget "$flat" schema)" = "session/v1" ] || {
      mj_doctrine_fail session "$(basename "$f")" "schema is '$(mj_yget "$flat" schema)', and this executable reads session/v1" "head -n 3 $(mj_rel "$f")"
      bad=1
    }
    local key
    for key in kind session_id started_at closed_at outcome; do
      [ -n "$(mj_yget "$flat" "$key")" ] || {
        mj_doctrine_fail session "$(basename "$f")" "lacks $key, which every record carries" "head -n 20 $(mj_rel "$f")"
        bad=1
      }
    done
    case "$(mj_yget "$flat" outcome)" in
      closed|interrupted|"") ;;
      *) mj_doctrine_fail session "$(basename "$f")" "outcome '$(mj_yget "$flat" outcome)' is neither closed nor interrupted" "head -n 20 $(mj_rel "$f")"; bad=1 ;;
    esac
    # A shared record names no absolute path: those are facts about a machine, and the
    # ledger, which is local, is where they belong (ADR 0014).
    absolute="$(awk -F= '$2 ~ /^\// { print $1 }' "$flat" | head -n 3 | tr '\n' ' ')"
    [ -n "$absolute" ] && {
      mj_doctrine_fail session "$(basename "$f")" "carries an absolute path in: ${absolute% }" "grep -n ': /' $(mj_rel "$f")"
      bad=1
    }
    id="$(mj_yget "$flat" session_id)"
    case " $ids " in
      *" $id "*) mj_doctrine_fail session "$id" "two records claim this identity" "grep -rln 'session_id: $id' $(mj_rel "$dir")"; bad=1 ;;
      *) ids="$ids $id" ;;
    esac
    rm -f "$flat"
  done
  # A record nobody committed reaches nobody. This section's own contract calls these
  # records "shared with every surface", and every one of those surfaces — the site page,
  # the Rust index, the registry, `session list` in another clone — reads the tracked tree.
  # So an untracked record is written, validated by the loop above, counted in the `ok`
  # line below, and invisible everywhere a reader would actually look for it.
  #
  # The lifecycle writes a record when an episode closes and nothing commits it, so the gap
  # is the default rather than an accident: four of fifteen records here sat untracked,
  # three of them written the same morning, while the section reported itself healthy.
  #
  # A warning and not a failure, for the reason `clone unpushed` is one: the moments between
  # writing a record and staging it are legitimate, and a check that is red every time an
  # episode ends is a check people learn to scroll past. What was missing was not severity —
  # it was any mention at all.
  local orphan="" orphans=0 name
  while read -r name; do
    [ -n "$name" ] || continue
    orphans=$((orphans + 1))
    [ "$orphans" -le 3 ] && orphan="$orphan $name"
  done <<EOF
$(cd "$dir" 2>/dev/null && git ls-files --others --exclude-standard -- '*.md' 2>/dev/null)
EOF
  [ "$orphans" -gt 0 ] && mj_warn session "$(mj_rel "$dir")/" \
    "$orphans record(s) exist here and are committed nowhere, so they reach no surface:${orphan}$( [ "$orphans" -gt 3 ] && printf ' …' )" \
    "git add $(mj_rel "$dir") && majordomus derive"
  [ "$n" -gt 0 ] && [ "$bad" = 0 ] \
    && mj_doctrine_ok session "$(mj_rel "$dir")/" "$n record(s) — every field the contract has, no conversation, no absolute path" "majordomus session list"
  return 0
}

#!/usr/bin/env bash
# sourced by session, capture and doctor; guard against re-sourcing
[ -n "${MJ_LIB_session_context:-}" ] && return 0 || MJ_LIB_session_context=1
# session_context — the bounded working context of one episode, written when it opens.
#
# The layer has always named this store and never filled it. That was the defect: a
# directory the skeleton creates and nothing produces is a promise the layer does not keep,
# and a reader cannot tell an empty store from an unimplemented one.
#
# What belongs here follows from what the two neighbouring stores already are. The closed
# session record under .ai/repo/sessions/ is shared, derived and immutable — it says what
# the episode produced. The prompt archive under .ai/local/prompts/ is the person's half of
# the exchange, raw and local. Neither of them says what the worker was actually told at the
# open, and that is the fact this store keeps: the context the builder resolved at the
# moment the episode began, frozen.
#
# Frozen is the right shape for a snapshot and the wrong shape for a working context, and
# for a long time this store was asked to be both. It carried a `## Notes` heading for the
# worker to hand-edit; measured on 2026-09-11 across the 23 episodes this checkout has
# opened, 23 of 23 carried the template and 0 carried a note. A lifecycle that depends on a
# model remembering to edit a Markdown section is not a lifecycle, and a heading with no
# producer is a database nobody writes to.
#
# So the two are separated. The document here is the opening snapshot and nothing else, and
# the *working* context is not a document at all: `mj_session_context_render` composes it on
# every read from the episode's own ledger window, from git, and from this snapshot's
# provenance. Nothing has to remember to refresh it, because nothing refreshes it — it is
# derived, so it cannot be stale. What the worker records with `majordomus decision`,
# `majordomus question` and `majordomus checkpoint` reaches it because `mj_ledger_append`
# already stamps every one of those lines with the open episode's id. No fourth noun was
# invented to carry a note (project.no-new-nouns): the three that exist already have
# commands, ledger events and episode attribution, and what they lacked was a reader.
#
# It is local, and stays local, for two reasons that are not the same one. The body names
# this machine — the worktree path, the checkpoint that was newest here — and a fact about a
# disk is not a fact about the repository (ADR 0014). And it is a snapshot of a projection:
# regenerating it later gives a different document, so publishing it would publish something
# no surface can reproduce.
#
# What it must never become is a transcript. The rule project.never-store-transcripts holds
# here exactly as it holds for a checkpoint: the derived half is written by the tool from
# durable state, and the authored half is the worker's own summary of the work — a statement
# about the repository, not a retelling of a conversation with a model. The validator below
# refuses the field names a transcript would arrive in, so the refusal is mechanical rather
# than remembered.

# The builder this store freezes. session.sh is not sourced here: it sources this file, and
# the two validators below run only under doctor, which has both.
# shellcheck source=context.sh
. "$MJ_LIB_DIR/context.sh"

MJ_SESSION_CONTEXT_SCHEMA="session-context/v1"
# The closed set of front-matter keys a document carries. A key outside it is a defect the
# session_lifecycle doctrine reports, for the same reason the prompt archive declares its
# fields: a store whose shape is open cannot be validated, and a document that grew a field
# nobody declared is how a transcript arrives one key at a time.
MJ_SESSION_CONTEXT_KEYS="schema kind session_id opened_at opened_by provider provider_session branch head task_id profile worker"

# Where the documents live. MJ_SESSION_CONTEXT_DIR overrides it so a verifier can drive the
# writer without writing into the store it is checking.
mj_session_context_dir() {
  if [ -n "${MJ_SESSION_CONTEXT_DIR:-}" ]; then printf '%s' "$MJ_SESSION_CONTEXT_DIR"
  else printf '%s/session-contexts' "$MJ_AI_LOCAL_DIR"; fi
}

# The document of one session id, or nothing. The name carries the stamp first so a listing
# is chronological, and the session id second so this lookup is a glob rather than a scan.
mj_session_context_path() {
  local dir; dir="$(mj_session_context_dir)"
  [ -d "$dir" ] || return 0
  find "$dir" -maxdepth 1 -name "*--$1.md" 2>/dev/null | LC_ALL=C sort | tail -n 1
}

# ---------------------------------------------------------------- open
# mj_session_context_open <session-id> <opened-by> <provider> <provider-session> <worker>
# Writes the document for an episode that has just opened. Prints its repository-relative
# path, or nothing when the store cannot be written — an episode opens either way, because
# a working context that failed to be written is worth less than the session it describes.
mj_session_context_open() {
  local sid="$1" by="$2" provider="${3:-}" psession="${4:-}" worker="${5:-}" dir out tmp task profile
  dir="$(mj_session_context_dir)"
  mkdir -p "$dir" 2>/dev/null || { mj_session_context_log "cannot create $(mj_rel "$dir")"; return 0; }
  out="$dir/$(mj_now_compact)--$sid.md"
  tmp="$out.mj-tmp"

  task=none; profile=none
  if mj_load_current; then task="$(mj_cur id)"; profile="$(mj_cur profile)"; fi

  {
    printf -- '---\n'
    printf 'schema: %s\nkind: session-context\n' "$MJ_SESSION_CONTEXT_SCHEMA"
    printf 'session_id: %s\nopened_at: %s\nopened_by: %s\n' "$sid" "$(mj_now)" "$by"
    # Absent stays absent, as it does in the open session record: a provider nobody named is
    # not inferred, and the identity of a provider's own session is quoted because it is the
    # provider's string and not this tool's.
    [ -n "$provider" ] && printf 'provider: %s\n' "$provider"
    [ -n "$psession" ] && printf 'provider_session: "%s"\n' "$(printf '%s' "$psession" | sed 's/"/\\"/g')"
    printf 'branch: %s\nhead: %s\ntask_id: %s\nprofile: %s\n' \
      "$(mj_git_branch)" "$(mj_git_head)" "$task" "$profile"
    [ -n "$worker" ] && printf 'worker: "%s"\n' "$(printf '%s' "$worker" | sed 's/"/\\"/g')"
    printf -- '---\n\n'
    printf '# Opening snapshot of session %s\n\n' "$sid"
    printf 'The context below is the builder'"'"'s output at the moment this episode opened,\n'
    printf 'frozen. It is evidence of what the worker was told, not a source of truth: every\n'
    printf 'line of it is checked against git, and `majordomus context` re-resolves it.\n\n'
    printf 'This file is the snapshot alone. The episode'"'"'s *working* context — what is true\n'
    printf 'now, including everything the episode has recorded since it opened — is not stored\n'
    printf 'here and is not a file: `majordomus session context` composes it on every read.\n\n'
    printf '## Context at open\n\n'
    mj_session_context_body
  } > "$tmp" 2>/dev/null || { rm -f "$tmp"; mj_session_context_log "cannot write $(mj_rel "$out")"; return 0; }
  mv "$tmp" "$out" 2>/dev/null || { rm -f "$tmp"; return 0; }
  printf '%s\n' "${out#"$MJ_ROOT/"}"
}

# The builder's body, assembled in process. The command itself exits when it is done, so the
# two stages it is made of are called directly: the sections, then the render. A context that
# cannot be assembled — an unreadable policy, a profile that names nothing — leaves a line
# saying so rather than an empty section, because a blank heading reads as "there was no
# context" when what happened is that the builder failed.
mj_session_context_body() {
  local budget doc
  if ! mj_load_policy; then
    printf 'The context builder did not run: the policy does not parse (run: majordomus doctor).\n'
    return 0
  fi
  budget="$(mj_pol context.builder_budget_lines)"
  case "$budget" in ''|*[!0-9]*) budget=200 ;; esac
  MJ_CTX_TMP="$(mktemp -d "${TMPDIR:-/tmp}/mj.sctx.XXXXXX")"
  doc="$MJ_CTX_TMP/doc"
  if mj_context_sections "" 1 2>/dev/null && mj_ctx_render "$doc" "$budget" "" "" "" 2>/dev/null; then
    cat "$doc"
  else
    printf 'The context builder did not run to completion here (run: majordomus context).\n'
  fi
  rm -rf "$MJ_CTX_TMP"
  MJ_CTX_TMP=""
}

# ---------------------------------------------------------------- render
# The working context of an episode, composed at the moment it is asked for.
#
# This is the answer to "what is true for this episode now", and it is deliberately not a
# file. The store beside it holds the opening snapshot, which is frozen because a snapshot
# that changes is not evidence of anything; this is derived on every read, because a working
# context that is written once is a snapshot wearing the wrong name. Between them there is
# no third artefact to keep in step, and therefore nothing that can go stale — the shape the
# mandate asked for (events + repository state -> context) with the materialisation left
# out, because at this size the projection costs one awk pass over a ledger window and a
# cache would only add a way to be wrong.
#
# Its three sources are the three that can be checked:
#
#   * the open episode's own record — identity, when it opened, who opened it, and the head
#     it opened at. That record is what `session start` wrote and what every later command
#     resolves through.
#   * git, now. The branch and head now against the branch and head at open, the commits in
#     between, and whether the tree is dirty.
#   * the episode's ledger window — every line `mj_ledger_append` stamped with this session
#     id. That is where the worker's own records are: checkpoints, decisions, questions,
#     handovers, evidence. They are already episode-attributed; nothing here infers which
#     episode wrote what from a timestamp, which is the mistake the window exists to avoid.
#
# It does not re-resolve the context builder. `majordomus context` is that capability and
# duplicating it here would be a second definition of one semantic, which this repository
# treats as a design defect rather than a convenience (ADR 0004). What this prints instead
# is the episode; what the builder resolves is the repository.
#
# It never fabricates. A section with nothing in it says so and names the command that would
# have put something there, for the same reason the closed record's body does: an
# acknowledged gap is a fact a reader can act on, and an invented summary is one nobody can
# tell from a real one afterwards.

# A multi-line document as one JSON string. `mj_json_esc` is for scalars and ends with
# `tr -d '\n'`, which deletes line breaks rather than escaping them — correct for a title,
# silently destructive for a document, which would arrive as one run-on line. This escapes
# them instead, so the JSON form of the context is the same document the text form is.
mj_json_esc_lines() {
  LC_ALL=C awk '
    { gsub(/\\/, "\\\\"); gsub(/"/, "\\\""); gsub(/\t/, "\\t"); gsub(/\r/, "")
      printf "%s%s", sep, $0; sep = "\\n" }
    END { printf "" }'
}

# mj_session_context_render <session-id>
# Writes the composed working context to standard output. Returns 0 even where a source is
# missing: a context assembled from two of three sources is worth more than a failure.
mj_session_context_render() {
  local sid="$1" snap win opened_at opened_by provider psession worker task profile
  local start_head start_branch head_now branch_now n_commits n_files

  snap="$(mj_session_context_path "$sid")"
  win="$(mktemp "${TMPDIR:-/tmp}/mj.swin.XXXXXX")"
  if command -v mj_session_window >/dev/null 2>&1; then
    mj_session_window "$sid" > "$win" 2>/dev/null || : > "$win"
  else : > "$win"; fi

  # The episode's own facts come from the open record when this episode is the open one,
  # and from the snapshot's front matter otherwise — a context can be asked for by id after
  # the episode closed, and the answer then is still the episode's, not this checkout's.
  opened_at="$(mj_ses started_at 2>/dev/null || true)"
  opened_by="$(mj_ses opened_by 2>/dev/null || true)"
  provider="$(mj_ses provider 2>/dev/null || true)"
  psession="$(mj_ses provider_session 2>/dev/null || true)"
  worker="$(mj_ses worker 2>/dev/null || true)"
  start_head="$(mj_ses start_head 2>/dev/null || true)"
  start_branch="$(mj_ses branch 2>/dev/null || true)"
  # The snapshot fills what the open record does not carry, and answers entirely for an
  # episode that is no longer the open one — a context can be asked for by id after its
  # episode closed, and the answer then is still that episode's, not this checkout's.
  # `opened_by` is always one of these: the open record has never carried it.
  if [ -n "$snap" ]; then
    [ -z "$opened_at" ]   && opened_at="$(mj_session_context_fm "$snap" opened_at)"
    [ -z "$opened_by" ]   && opened_by="$(mj_session_context_fm "$snap" opened_by)"
    [ -z "$provider" ]    && provider="$(mj_session_context_fm "$snap" provider)"
    [ -z "$worker" ]      && worker="$(mj_session_context_fm "$snap" worker)"
    [ -z "$start_head" ]  && start_head="$(mj_session_context_fm "$snap" head)"
    [ -z "$start_branch" ] && start_branch="$(mj_session_context_fm "$snap" branch)"
  fi
  task=none; profile=none
  if mj_load_current 2>/dev/null; then task="$(mj_cur id)"; profile="$(mj_cur profile)"; fi

  head_now="$(mj_git_head)"; branch_now="$(mj_git_branch)"

  printf '# Working context of session %s\n\n' "$sid"
  printf 'Composed now, from this episode'"'"'s record, from git, and from the ledger lines this\n'
  printf 'episode wrote. It is not stored and is not a snapshot: every read recomposes it.\n\n'

  printf '## Episode\n\n'
  printf -- '- session: %s\n' "$sid"
  [ -n "$opened_at" ] && printf -- '- opened: %s%s\n' "$opened_at" "${opened_by:+ by $opened_by}"
  [ -n "$provider" ] && printf -- '- provider: %s%s\n' "$provider" "${psession:+ (session $psession)}"
  [ -n "$worker" ] && printf -- '- worker: %s\n' "$worker"
  printf -- '- task: %s (profile %s)\n' "$task" "$profile"
  if [ -n "$snap" ]; then printf -- '- opening snapshot: %s\n' "${snap#"$MJ_ROOT/"}"
  else printf -- '- opening snapshot: none was written; `majordomus doctor` reports why\n'; fi

  printf '\n## Repository now\n\n'
  printf -- '- branch: %s' "$branch_now"
  [ -n "$start_branch" ] && [ "$start_branch" != "$branch_now" ] && printf ' (opened on %s)' "$start_branch"
  printf '\n'
  printf -- '- head: %s' "$(printf '%.7s' "$head_now")"
  if [ -n "$start_head" ] && [ "$start_head" != NONE ] && [ "$start_head" != "$head_now" ]; then
    printf ' (opened at %s)' "$(printf '%.7s' "$start_head")"
  fi
  printf '\n'
  printf -- '- working tree: %s\n' "$(mj_git_dirty)"
  if [ -n "$start_head" ] && [ "$start_head" != NONE ] && mj_git merge-base --is-ancestor "$start_head" HEAD 2>/dev/null; then
    n_commits="$(mj_git rev-list --count "$start_head..HEAD" 2>/dev/null || printf 0)"
    n_files="$(mj_git diff --name-only "$start_head" HEAD 2>/dev/null | grep -c . || true)"
    printf -- '- this episode: %s commit(s), %s file(s) changed\n' "$n_commits" "${n_files:-0}"
  elif [ -n "$start_head" ] && [ "$start_head" != NONE ]; then
    printf -- '- this episode: the history diverged from the commit it opened at (%s), so its commits cannot be counted\n' "$(printf '%.7s' "$start_head")"
  fi

  printf '\n## Recorded in this episode\n\n'
  printf 'What the worker itself put on the record. These are the three commands that write\n'
  printf 'here; nothing infers an entry from a diff, and a section with nothing in it means\n'
  printf 'nothing was recorded, not that nothing happened.\n\n'
  mj_session_context_recorded "$win"

  printf '\n## The repository'"'"'s own context\n\n'
  printf 'Not repeated here: `majordomus context` resolves it, now, against the current tree.\n'
  [ -n "$snap" ] && printf 'What this episode was told at its open is under `## Context at open` in %s.\n' "${snap#"$MJ_ROOT/"}"

  rm -f "$win"
  return 0
}

# One front-matter scalar of a snapshot, unquoted. Small enough to stay here rather than
# grow a parser: the snapshot's key set is closed and declared above.
mj_session_context_fm() {
  awk -v k="$2" '
    FNR == 1 { if ($0 != "---") exit; next }
    $0 == "---" { exit }
    index($0, k ":") == 1 { v = substr($0, length(k) + 2); sub(/^ +/, "", v); gsub(/^"|"$/, "", v); print v; exit }' "$1" 2>/dev/null
}

# The four kinds of authored record, read back out of the episode's window. Each one names
# the command that fills it, so that a worker reading its own empty context is told what to
# do rather than left to infer that the section is decorative — which is exactly what the
# `## Notes` template it replaces failed to do, 23 times out of 23.
mj_session_context_recorded() {
  local win="$1" list f body n

  printf '### Progress notes\n\n'
  list="$(mj_session_context_win "$win" 'task.checkpoint' checkpoint_path)"
  if [ -z "$list" ]; then
    printf 'None. `majordomus checkpoint` records what this episode is doing and why, and is\n'
    printf 'what puts a note of the worker'"'"'s own into this context and into the closed record.\n'
  else
    local IFS='
'
    for f in $list; do
      [ -n "$f" ] || continue
      if [ -f "$MJ_ROOT/$f" ]; then
        body="$(mj_record_body "$MJ_ROOT/$f" | sed '/^$/d' | head -n 8)"
        [ -n "$body" ] && printf -- '- %s\n' "$(printf '%s' "$body" | head -n 1)" && \
          printf '%s\n' "$body" | tail -n +2 | sed 's/^/  /'
      else
        printf -- '- (a checkpoint was recorded and its file is no longer present)\n'
      fi
    done
    unset IFS
  fi

  printf '\n### Decisions\n\n'
  mj_session_context_lines "$win" 'decision.recorded' decision \
    'None. `majordomus decision add "<what>" --why "<why>"` records one; nothing infers a decision from a diff.'

  printf '\n### Open questions\n\n'
  mj_session_context_lines "$win" 'question.opened|question.resolved' question \
    'None. `majordomus question add "<question>"` opens one, and `question resolve` closes it.'

  printf '\n### Continuation records\n\n'
  n="$(mj_session_context_win "$win" 'task.handed_over' handover_path | grep -c . || true)"
  if [ "${n:-0}" = 0 ]; then
    printf 'None. `majordomus handover` writes the record the next worker starts from.\n'
  else
    printf '%s written in this episode; `majordomus handover show` reads the latest.\n' "$n"
  fi
  return 0
}

# Values of one field over the window, as a list, or the given sentence when there are none.
mj_session_context_lines() {
  local win="$1" ev="$2" key="$3" empty="$4" list v
  list="$(mj_session_context_win "$win" "$ev" "$key")"
  if [ -z "$list" ]; then printf '%s\n' "$empty"; return 0; fi
  local IFS='
'
  for v in $list; do [ -n "$v" ] && printf -- '- %s\n' "$v"; done
  unset IFS
  return 0
}

# mj_session_field belongs to the session module, which is what calls this; the guard keeps
# this file sourceable on its own, as doctor and capture source it without session.sh.
mj_session_context_win() {
  command -v mj_session_field >/dev/null 2>&1 || return 0
  [ -f "$1" ] || return 0
  mj_session_field "$1" "$2" "$3"
}

# ---------------------------------------------------------------- close
# mj_session_context_close <session-id> <outcome> <record-path>
# Appends what the close knows to the document the open wrote. The front matter is not
# rewritten: it describes the open, and a file that is only ever appended to cannot lose
# what the worker typed into it between the two events.
mj_session_context_close() {
  local sid="$1" outcome="$2" record="$3" out
  out="$(mj_session_context_path "$sid")"
  [ -n "$out" ] || return 0
  { printf '\n## Close\n\n'
    printf -- '- closed_at: %s\n- outcome: %s\n' "$(mj_now)" "$outcome"
    printf -- '- head: %s\n' "$(mj_git_head)"
    [ -n "$record" ] && printf -- '- record: %s\n' "$record"
  } >> "$out" 2>/dev/null || mj_session_context_log "cannot append the close to $(mj_rel "$out")"
  printf '%s\n' "${out#"$MJ_ROOT/"}"
}

# A failure of this store is never allowed to cost an episode — the writer runs inside
# `session start`, which itself runs inside a provider hook — so it fails into a log beside
# the documents, and the doctrine below reports a non-empty log. Same argument as the
# capture log, same consequence: without it the store stops filling in silence.
mj_session_context_log() {
  local dir; dir="$(mj_session_context_dir)"
  mkdir -p "$dir" 2>/dev/null || return 0
  printf '%s %s\n' "$(mj_now)" "$1" >> "$dir/.session-context.log" 2>/dev/null || true
}

# ---------------------------------------------------------------- doctrine
# The store's invariants, and the boundary that keeps it local. What the wiring verifier
# proves is that the provider opens and closes the episode; this proves that what the open
# wrote is what the contract allows, and that it never left the machine.
mj_validate_session_lifecycle() {
  local dir rel n tracked
  dir="$(mj_session_context_dir)"; rel="$(mj_rel "$dir")"

  [ -d "$dir" ] || { mj_doctrine_ok session "$rel" "no working contexts here; no episode has opened in this checkout"; return 0; }

  tracked="$(mj_git ls-files -- "$rel" 2>/dev/null | head -n 3 | tr '\n' ' ')"
  if [ -n "$tracked" ]; then
    mj_doctrine_fail session "$rel" "working contexts are tracked by git: $tracked" "git rm --cached -r $rel"
  elif ! mj_git check-ignore -q "$dir" 2>/dev/null; then
    mj_doctrine_fail session "$rel" "is not ignored, so a working context can be committed" "grep -n '.ai/local/' .gitignore"
  else
    mj_doctrine_ok session "$rel" "ignored and untracked"
  fi

  mj_session_context_failures "$dir" "$rel"

  n="$(find "$dir" -maxdepth 1 -name '*.md' 2>/dev/null | wc -l | tr -d ' ')"
  [ "$n" -gt 0 ] || { mj_doctrine_ok session "$rel" "no working contexts yet; the store is empty"; return 0; }
  mj_session_context_documents "$dir" "$rel" "$n"
  mj_session_context_open_episode "$rel"
  return 0
}

mj_session_context_failures() {
  local dir="$1" rel="$2" n
  local log="$dir/.session-context.log"
  [ -s "$log" ] || return 0
  n="$(grep -c . "$log" 2>/dev/null || true)"
  mj_doctrine_fail session "$rel/.session-context.log" \
    "$n working context(s) could not be written; the most recent: $(tail -n 1 "$log")" \
    "cat $rel/.session-context.log   # then remove it once understood"
}

# Every document opens with the contract, carries only the declared keys, and carries none
# of the field names a transcript would arrive in. One awk pass over the store, batched by
# find, so the cost is the store's size and not a process per document.
mj_session_context_documents() {
  local dir="$1" rel="$2" n="$3" out shape unknown transcript
  out="$(find "$dir" -maxdepth 1 -name '*.md' -exec awk -v keys=" $MJ_SESSION_CONTEXT_KEYS " '
    FNR == 1 { fm = ($0 == "---") ? 1 : 0; ok = 0; if (!fm) print "SHAPE " FILENAME; next }
    fm == 1 && $0 == "---" { fm = 2; if (!ok) print "SHAPE " FILENAME; next }
    fm == 1 {
      if ($0 ~ /^[a-z_]+:/) {
        k = $0; sub(/:.*/, "", k)
        if (index(keys, " " k " ") == 0) print "UNKNOWN " FILENAME " " k
        if (k == "schema" && $0 ~ /session-context\/v1/) ok = 1
      }
      next }
    fm == 2 && /^(transcript|messages|assistant|completion|response)[: ]/ { if (!t[FILENAME]++) print "TRANSCRIPT " FILENAME }
  ' {} + 2>/dev/null)"
  shape="$(printf '%s' "$out" | grep -c '^SHAPE ' || true)"
  unknown="$(printf '%s' "$out" | grep -c '^UNKNOWN ' || true)"
  transcript="$(printf '%s' "$out" | grep -c '^TRANSCRIPT ' || true)"
  if [ "$transcript" != 0 ]; then
    mj_doctrine_fail session "$rel" "$transcript working context(s) carry a conversation: $(printf '%s' "$out" | sed -n 's/^TRANSCRIPT .*\///p' | head -n 3 | tr '\n' ' ')" \
      "grep -lE '^(transcript|messages|assistant|completion|response)[: ]' $rel/*.md"
  elif [ "$shape" != 0 ]; then
    mj_doctrine_fail session "$rel" "$shape working context(s) do not open with $MJ_SESSION_CONTEXT_SCHEMA front matter: $(printf '%s' "$out" | sed -n 's/^SHAPE .*\///p' | head -n 3 | tr '\n' ' ')" \
      "head -n 3 $rel/*.md"
  elif [ "$unknown" != 0 ]; then
    mj_doctrine_fail session "$rel" "$unknown undeclared front-matter key(s): $(printf '%s' "$out" | sed -n 's/^UNKNOWN .*\/\([^ ]*\) \(.*\)$/\1 \2/p' | head -n 3 | tr '\n' ' ')" \
      "the declared set is: $MJ_SESSION_CONTEXT_KEYS"
  else
    mj_doctrine_ok session "$rel" "$n working context(s), each one $MJ_SESSION_CONTEXT_SCHEMA and none carrying a conversation"
  fi
}

# An open episode with no working context is the producer having failed: the document is
# written when the session opens, so its absence means either the store broke or the record
# was made by something other than this tool.
mj_session_context_open_episode() {
  local rel="$1" sid
  mj_load_session >/dev/null 2>&1 || return 0
  mj_session_is_foreign && return 0
  sid="$(mj_ses session_id)"
  [ -n "$sid" ] || return 0
  if [ -n "$(mj_session_context_path "$sid")" ]; then
    mj_doctrine_ok session "$sid" "the open episode has its working context under $rel"
  else
    mj_doctrine_fail session "$sid" "the open episode has no working context under $rel" \
      "cat $rel/.session-context.log 2>/dev/null; majordomus session status"
  fi
}

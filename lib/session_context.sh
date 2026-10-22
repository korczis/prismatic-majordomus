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
# moment the episode began, frozen, next to the notes the worker adds while working.
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

# ---------------------------------------------------------------- freshness
# One front-matter field of a document, read without parsing the body. The front matter is
# a closed set of plain `key: value` lines (MJ_SESSION_CONTEXT_KEYS above), so the reader
# stops at the closing fence rather than scanning a document whose body is a whole context.
mj_session_context_field() {
  awk -v k="$2" '
    FNR == 1 { if ($0 != "---") exit; next }
    $0 == "---" { exit }
    index($0, k ": ") == 1 { print substr($0, length(k) + 3); exit }
  ' "$1" 2>/dev/null
}

# mj_session_context_freshness <session-id>
# How far the frozen briefing has drifted from the checkout it describes.
#
# Every other artefact in this repository that records a head is compared against git before
# it is believed. The task record is (mj_validate_state, and the GIT section of the context
# builder), so is the resolved handover, so is the checkpoint, so is the derivation's start
# head, and so is the closed session record, which `session show` labels for exactly this
# reason. They all speak one vocabulary — `exact`, `advanced`, `diverged`,
# `different_context` — and they all get it from mj_git_label.
#
# The working context was the single artefact carrying a recorded head that nobody compared,
# and it is the one the worker is actually holding: the briefing a provider hook delivered
# once, at the open, and never again. So a session could work all afternoon from a briefing
# whose repository had moved underneath it while `context`, `check` and `doctor` each stayed
# quiet — a relation checked in every direction but the one that mattered.
#
# Nothing here refreshes the document. It is frozen evidence of what the worker was told,
# and a store that rewrote its own history underneath an earlier prompt would be worth less
# than one that admits its age. What was missing was the age, not a mutation.
#
# Prints: <label> <TAB> <recorded-head> <TAB> <commits-since>, and returns 1 when the
# session has no document at all — which is a different fact, and one the open-episode
# check above already reports.
mj_session_context_freshness() {
  local sid="$1" doc head branch label since=0
  doc="$(mj_session_context_path "$sid")"
  [ -n "$doc" ] || return 1
  head="$(mj_session_context_field "$doc" head)"
  branch="$(mj_session_context_field "$doc" branch)"
  # A document without both fields cannot be compared, and `unknown` is the vocabulary's own
  # word for that: it is not `exact`, and reporting it as `exact` is how a stale briefing
  # would come to look current.
  if [ -z "$head" ] || [ -z "$branch" ]; then
    printf 'unknown\t%s\t0\n' "${head:-NONE}"
    return 0
  fi
  label="$(mj_git_label "$head" "$branch")"
  # The count is only meaningful along one line of history: `diverged` and
  # `different_context` are not a distance, and a number printed for them would invite the
  # reader to treat them as one.
  if [ "$label" = advanced ]; then
    since="$(mj_git rev-list --count "$head..HEAD" 2>/dev/null || printf 0)"
    case "$since" in ''|*[!0-9]*) since=0 ;; esac
  fi
  printf '%s\t%s\t%s\n' "$label" "$head" "$since"
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
    printf '# Working context of session %s\n\n' "$sid"
    printf 'The context below is the builder'"'"'s output at the moment this episode opened,\n'
    printf 'frozen. It is evidence of what the worker was told, not a source of truth: every\n'
    printf 'line of it is checked against git, and `majordomus context` re-resolves it.\n\n'
    printf '## Context at open\n\n'
    mj_session_context_body
    printf '\n## Notes\n\n'
    printf 'The worker'"'"'s own notes about the repository — what was decided, what is left,\n'
    printf 'what the next episode needs. Not a retelling of the conversation: the prompts have\n'
    printf 'their own store, and a summary of a dialogue is not a fact this repository can check.\n'
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
  if [ -z "$(mj_session_context_path "$sid")" ]; then
    mj_doctrine_fail session "$sid" "the open episode has no working context under $rel" \
      "cat $rel/.session-context.log 2>/dev/null; majordomus session status"
    return 0
  fi

  # The document exists; say how far it has drifted from the checkout it describes, in the
  # same vocabulary mj_validate_state uses for the task record. Drift is reported, never
  # refused: a briefing is `advanced` the instant its own session makes a commit, and
  # `different_context` the instant the worker moves to the branch it was told to build, so
  # a doctrine that failed on either would stop every commit in the repository. The defect
  # this closes is that nobody said the number at all. `unknown` is the exception and does
  # fail, because a document this tool wrote always carries both fields: without them the
  # producer is broken, and a briefing that cannot be compared must not read as a current one.
  local fresh label head since
  fresh="$(mj_session_context_freshness "$sid")" || return 0
  label="${fresh%%	*}"; fresh="${fresh#*	}"
  head="${fresh%%	*}"; since="${fresh#*	}"
  case "$label" in
    unknown)
      mj_doctrine_fail session "$sid" \
        "the open episode's working context carries no head and branch, so it cannot be compared with git" \
        "head -n 12 $(mj_rel "$(mj_session_context_path "$sid")")" ;;
    advanced)
      mj_doctrine_ok session "$sid" \
        "working context under $rel, $label: $since commit(s) since the briefing was frozen at ${head:0:7}" ;;
    exact)
      mj_doctrine_ok session "$sid" "working context under $rel, $label at ${head:0:7}" ;;
    *)
      mj_doctrine_ok session "$sid" \
        "working context under $rel, $label from ${head:0:7}; re-resolve it with: majordomus context" ;;
  esac
}

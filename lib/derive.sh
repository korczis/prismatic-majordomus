#!/usr/bin/env bash
# sourced by checkpoint, handover and session; guard against re-sourcing
[ -n "${MJ_LIB_derive:-}" ] && return 0 || MJ_LIB_derive=1
# derive — compose a checkpoint or a handover body from records that already exist.
#
# Every continuation record in this tool takes its body on stdin, which is right when a
# worker is writing one deliberately and fatal when nothing is. A lifecycle that only runs
# because somebody remembered to run it is not a lifecycle, and the two repositories this
# design was compared against both stop exactly here: their protocols are complete, their
# recovery is documented, and both leave writing the record to a worker's memory. What is
# missing in each is not a format. It is a body that exists without one being typed.
#
# So the sections are composed from the ledger, the task record, git, the open questions and
# the newest checkpoint — all of which are already written, already validated, and already
# identity-checked. Nothing here calls a model, makes a network call, or summarises prose.
# A derived body is a projection of records, which is why it is allowed to be written by a
# hook: it can be wrong only if a record it reads is wrong, and that record has its own gate.
#
# The distinction the tool already draws is preserved. A derived checkpoint is short by the
# same policy cap as a typed one, and says what moved. A derived handover carries the
# policy's required sections and says what the next worker must do. Neither replaces an
# authored body: `handover` still reads stdin when given one, and a worker that writes its
# own says more than this can. What derivation guarantees is that the absence of a worker
# willing to type is no longer the same thing as the absence of a record.

# Everything this file reads, it sources: a generator that works only when some other
# command happened to have loaded its dependency is a generator that fails in a hook.
# shellcheck source=session.sh
. "$MJ_LIB_DIR/session.sh"
# shellcheck source=decision.sh
. "$MJ_LIB_DIR/decision.sh"
# shellcheck source=question.sh
. "$MJ_LIB_DIR/question.sh"

# How many items any derived list prints before it says how many it left out. The cap is
# the policy's, not a constant here: a derived body is read into the next worker's context
# and is bounded by the same number that bounds everything else offered to one.
mj_derive_cap() {
  local n; n="$(mj_pol context.max_list_items)"
  case "$n" in ''|*[!0-9]*) n=20 ;; esac
  printf '%s' "$n"
}

# Non-blank lines held in a variable. `printf '%s' "$v" | wc -l` counts newlines, so it is
# one short whenever the value does not end in one, which for a captured command
# substitution is always: the count and the list beneath it then disagree by one, in a
# record whose whole purpose is to be trusted without re-deriving it.
mj_derive_nlines() {
  printf '%s\n' "$1" | sed '/^$/d' | wc -l | tr -d ' '
}

# Print at most N lines of stdin, then one line naming what was dropped. Truncation is
# always visible: a list that silently ends is indistinguishable from a list that ended.
mj_derive_bounded() {
  local cap="$1" noun="$2" tmp total
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.dv.XXXXXX")"; cat > "$tmp"
  total="$(mj_lines "$tmp")"
  head -n "$cap" "$tmp"
  [ "$total" -gt "$cap" ] && printf -- '- ... and %s more %s\n' "$((total - cap))" "$noun"
  rm -f "$tmp"
  return 0
}

# The files this task has touched: everything git reports as changed since the commit the
# task started at, plus the working tree's own changes. Names only — a derived record
# references the diff, it does not carry one.
mj_derive_changed() {
  local base="${1:-}" tmp
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.dc.XXXXXX")"
  : > "$tmp"
  if [ -n "$base" ] && [ "$base" != NONE ] && mj_git merge-base --is-ancestor "$base" HEAD 2>/dev/null; then
    mj_git diff --name-only "$base..HEAD" 2>/dev/null >> "$tmp"
  fi
  mj_git status --porcelain=v1 2>/dev/null | cut -c4- | sed 's/^.* -> //' >> "$tmp"
  LC_ALL=C sort -u "$tmp" | sed '/^$/d'
  rm -f "$tmp"
  return 0
}

# Count of ledger lines of one event kind belonging to this episode, or to this task when
# no session is open. Both selectors are stamped by the writer; neither is a time range.
mj_derive_count() {
  local ev="$1" sid="$2" task="$3" led="$MJ_STATE_DIR/ledger.jsonl"
  [ -f "$led" ] || { printf '0'; return 0; }
  awk -v ev="$ev" -v sid="$sid" -v task="$task" '
    index($0, "\"event\":\"" ev "\"") == 0 { next }
    { if (sid != "") { if (index($0, "\"session\":\"" sid "\"")) n++ }
      else if (task != "") { if (index($0, "\"task_id\":\"" task "\"")) n++ }
      else n++ }
    END { printf "%d", n + 0 }' "$led"
}

# The unresolved questions, as their own text.
#
# Through `mj_question_unresolved_any` and not through an awk of its own, because that is the
# reader the acceptance gate uses, and a derived record whose blocker list disagreed with the
# gate's would be worse than one with no blocker list at all. The first version here did have
# its own awk, and it reported the example line inside the store template's HTML comment as a
# real blocker in every fresh checkout — a phantom question, in the one section a resuming
# worker is told to act on before anything else.
mj_derive_questions() {
  local f; f="$(mj_question_file 2>/dev/null || printf '%s/open-questions.md' "$MJ_STATE_DIR")"
  [ -f "$f" ] || return 0
  mj_question_unresolved_any "$f" | sed -e 's/^[0-9]*:- \[unresolved\] /- /'
}

# ---------------------------------------------------------------- section writers
# One writer per section name. The dispatcher below calls the writer whose name matches the
# section, so a policy that adds a required section either has a writer here or is refused
# with the name of the section that has none — never silently emitted empty, which would
# pass the section gate while telling the next worker nothing.
#
# The section names are the policy's words, so the function names are derived from them by
# lowercasing and replacing every run of non-alphanumerics with one underscore.
mj_derive_slug() {
  printf '%s' "$1" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9]\{1,\}/_/g; s/^_//; s/_$//'
}

mj_derive_sec_objective() {
  printf '%s\n' "$MJ_DV_TASK_TEXT"
  printf '\nProfile %s. Declared scope: %s.\n' "$MJ_DV_PROFILE" "${MJ_DV_SCOPE:-(none)}"
  [ -n "$MJ_DV_ISSUES" ] && printf 'Issues in this episode: %s.\n' "$MJ_DV_ISSUES"
  return 0
}

mj_derive_sec_current_state() {
  local n label
  label="$(mj_git_label "$MJ_DV_START_HEAD" "$MJ_DV_BRANCH")"
  printf 'On %s at %s, working tree %s.\n' \
    "$MJ_DV_BRANCH" "$(printf '%.7s' "$MJ_DV_HEAD")" "$(mj_git_dirty)"
  [ "$MJ_DV_START_HEAD" != "$MJ_DV_HEAD" ] && \
    printf 'The task opened at %s; git has moved to %s since (%s).\n' \
      "$(printf '%.7s' "$MJ_DV_START_HEAD")" "$(printf '%.7s' "$MJ_DV_HEAD")" "$label"
  n="$(mj_derive_nlines "$MJ_DV_CHANGED")"
  if [ "$n" = 0 ]; then
    printf '\nNo file has changed since the task started.\n'
  else
    printf '\n%s file(s) changed since the task started:\n\n' "$n"
    printf '%s\n' "$MJ_DV_CHANGED" | sed '/^$/d' | sed 's/^/- /' | mj_derive_bounded "$(mj_derive_cap)" "file(s)"
  fi
  printf '\nRecorded in this episode: %s checkpoint(s), %s decision(s), %s question(s) opened, %s handover(s).\n' \
    "$MJ_DV_N_CHECK" "$MJ_DV_N_DEC" "$MJ_DV_N_Q" "$MJ_DV_N_HAND"
  if [ -n "$MJ_DV_LAST_CHECKPOINT" ]; then
    printf '\nThe newest checkpoint (%s), quoted whole:\n\n' "$MJ_DV_LAST_CHECKPOINT_AGE"
    mj_record_body "$MJ_DV_LAST_CHECKPOINT" | sed '/^$/d' | sed 's/^/> /' \
      | mj_derive_bounded "$(mj_pol_req checkpoint.max_body_lines)" "line(s)"
  fi
  return 0
}

mj_derive_sec_next_action() {
  # Ordered by what blocks what, not by preference. An open question refuses completion, so
  # it comes before verification; verification comes before the plan, because an issue moved
  # without it is a claim nobody checked.
  if [ -n "$MJ_DV_QUESTIONS" ]; then
    printf 'Answer the open question(s) below first: every unresolved entry on this branch refuses `majordomus finish --outcome completed`.\n\n'
    printf '%s\n' "$MJ_DV_QUESTIONS" | mj_derive_bounded "$(mj_derive_cap)" "question(s)"
    printf '\nThen run `majordomus check`, and `majordomus context` for the full briefing.\n'
    return 0
  fi
  printf 'Run `majordomus context` for the briefing this record is a projection of, then `majordomus check` to see whether the task is consistent with its scope and state.\n'
  if [ "$MJ_DV_N_CHECK" = 0 ]; then
    printf '\nNothing has been checkpointed in this episode, so the newest evidence of progress is git itself: read the diff named above before trusting any of it.\n'
  fi
  [ -n "$MJ_DV_ISSUES" ] && printf '\nThe issues this episode touched are %s; `majordomus plan next` says what follows them.\n' "$MJ_DV_ISSUES"
  return 0
}

mj_derive_sec_open_questions() {
  if [ -z "$MJ_DV_QUESTIONS" ]; then printf 'None. Nothing on this branch is blocking acceptance.\n'; return 0; fi
  printf '%s\n' "$MJ_DV_QUESTIONS" | mj_derive_bounded "$(mj_derive_cap)" "question(s)"
}

mj_derive_sec_decisions() {
  local f; f="$MJ_STATE_DIR/decisions.md"
  if [ ! -f "$f" ]; then printf 'None recorded in this checkout.\n'; return 0; fi
  local out; out="$(mj_decision_entries "$f" "" "$(mj_pol context.recent_decisions)" | awk '/^## /{ print "- " substr($0,4) }')"
  if [ -z "$out" ]; then printf 'None recorded in this checkout.\n'; return 0; fi
  printf '%s\n' "$out"
}

mj_derive_sec_verification() {
  # What a derived record may say about verification is what the ledger recorded, and
  # nothing else. A generator that wrote "tests pass" because it found a test directory
  # would be inventing the one field a reader most needs to be able to trust.
  local led="$MJ_STATE_DIR/ledger.jsonl" line
  if [ -f "$led" ]; then
    line="$(grep -F '"event":"task.finished"' "$led" 2>/dev/null | tail -n 1)"
    if [ -n "$line" ] && [ "$(mj_json_field "$line" task_id)" = "$MJ_DV_TASK_ID" ]; then
      printf 'The ledger records this task finished with outcome `%s`.\n' "$(mj_json_field "$line" outcome)"
      return 0
    fi
  fi
  printf 'Not run by this record. Nothing here asserts a test result; `majordomus finish --verify-command` is what records one.\n'
  return 0
}

# ---------------------------------------------------------------- gathering
# Read every input once, into variables the section writers expand. The rule the repository
# holds itself to is that derived state is computed once per state version rather than
# rediscovered per reader, and a body with six sections would otherwise ask git the same
# four questions six times.
MJ_DV_TASK_ID=""; MJ_DV_TASK_TEXT=""; MJ_DV_PROFILE=""; MJ_DV_SCOPE=""
MJ_DV_BRANCH=""; MJ_DV_HEAD=""; MJ_DV_START_HEAD=""; MJ_DV_CHANGED=""; MJ_DV_QUESTIONS=""
MJ_DV_N_CHECK=0; MJ_DV_N_DEC=0; MJ_DV_N_Q=0; MJ_DV_N_HAND=0; MJ_DV_ISSUES=""
MJ_DV_LAST_CHECKPOINT=""; MJ_DV_LAST_CHECKPOINT_AGE=""; MJ_DV_SESSION=""

mj_derive_gather() {
  MJ_DV_SESSION="$(mj_open_session_id)"
  MJ_DV_BRANCH="$(mj_git_branch)"; MJ_DV_HEAD="$(mj_git_head)"

  if mj_load_current; then
    MJ_DV_TASK_ID="$(mj_cur id)"; MJ_DV_TASK_TEXT="$(mj_cur task)"
    MJ_DV_PROFILE="$(mj_cur profile)"
    MJ_DV_START_HEAD="$(mj_cur head)"
    MJ_DV_SCOPE="$(mj_ylist "$MJ_CUR_FLAT" scope | tr '\n' ' ' | sed 's/ $//; s/ /, /g')"
  else
    MJ_DV_TASK_ID=none; MJ_DV_PROFILE=none; MJ_DV_START_HEAD="$MJ_DV_HEAD"
    MJ_DV_TASK_TEXT="No active task. This record describes an episode that worked outside one, which the tool permits and this line states rather than hides."
  fi

  MJ_DV_CHANGED="$(mj_derive_changed "$MJ_DV_START_HEAD")"
  MJ_DV_QUESTIONS="$(mj_derive_questions)"
  MJ_DV_N_CHECK="$(mj_derive_count task.checkpoint "$MJ_DV_SESSION" "$MJ_DV_TASK_ID")"
  MJ_DV_N_DEC="$(mj_derive_count decision.recorded "$MJ_DV_SESSION" "$MJ_DV_TASK_ID")"
  MJ_DV_N_Q="$(mj_derive_count question.opened "$MJ_DV_SESSION" "$MJ_DV_TASK_ID")"
  MJ_DV_N_HAND="$(mj_derive_count task.handed_over "$MJ_DV_SESSION" "$MJ_DV_TASK_ID")"

  # The issues this episode touched, from the ledger's own plan events.
  if [ -f "$MJ_STATE_DIR/ledger.jsonl" ] && [ -n "$MJ_DV_SESSION" ]; then
    MJ_DV_ISSUES="$(awk -v s="$MJ_DV_SESSION" '
      index($0, "\"session\":\"" s "\"") == 0 { next }
      index($0, "\"issue\":\"") == 0 { next }
      { v = $0; sub(/^.*"issue":"/, "", v); sub(/".*$/, "", v); print v }' \
      "$MJ_STATE_DIR/ledger.jsonl" | LC_ALL=C sort -u | tr '\n' ' ' | sed 's/ $//; s/ /, /g')"
  fi

  if [ "$MJ_DV_TASK_ID" != none ] && mj_resolve_latest "$MJ_STATE_DIR/checkpoints" "$MJ_DV_TASK_ID"; then
    MJ_DV_LAST_CHECKPOINT="$MJ_RES_PATH"
    MJ_DV_LAST_CHECKPOINT_AGE="$(mj_age_human "$(mj_age_minutes "$MJ_RES_CREATED" || true)")"
  fi
  return 0
}

# ---------------------------------------------------------------- handover body
# The sections the policy requires, in the order it names them, followed by the ones this
# generator can also fill. A required section with no writer is a configuration error
# reported by name; an optional one with no writer is simply not emitted.
MJ_DERIVE_OPTIONAL_SECTIONS="Decisions Open_Questions Verification"

mj_derive_handover_body() {
  mj_derive_gather
  local sec slug fn required emitted=" "
  required="$(mj_ylist "$MJ_POL_FLAT" handover.required_sections)"

  local IFS='
'
  for sec in $required; do
    [ -n "$sec" ] || continue
    slug="$(mj_derive_slug "$sec")"; fn="mj_derive_sec_$slug"
    command -v "$fn" >/dev/null 2>&1 || mj_die "$MJ_EX_CONTRACT" \
      "derive: the policy requires the section '$sec' and nothing derives it (add mj_derive_sec_$slug in lib/derive.sh, or write the body by hand)"
    printf '# %s\n\n' "$sec"; "$fn"; printf '\n'
    emitted="$emitted$sec "
  done
  unset IFS

  for sec in $MJ_DERIVE_OPTIONAL_SECTIONS; do
    sec="$(printf '%s' "$sec" | tr '_' ' ')"
    case "$emitted" in *" $sec "*) continue ;; esac
    slug="$(mj_derive_slug "$sec")"; fn="mj_derive_sec_$slug"
    command -v "$fn" >/dev/null 2>&1 || continue
    printf '# %s\n\n' "$sec"; "$fn"; printf '\n'
  done

  printf -- '---\n\nDerived by `majordomus handover --derive` from the task record, the ledger, git and the open questions. It asserts nothing that is not in one of those, and no model wrote it.\n'
  return 0
}

# ---------------------------------------------------------------- checkpoint body
# A checkpoint is a progress note, not a report, and the policy caps it. What moved is the
# whole content: the commit, the working tree, and how many files. Everything else a reader
# might want is one command away and named.
mj_derive_checkpoint_body() {
  mj_derive_gather
  local n; n="$(mj_derive_nlines "$MJ_DV_CHANGED")"
  printf 'At %s on %s, working tree %s; %s file(s) changed since the task started.\n' \
    "$(printf '%.7s' "$MJ_DV_HEAD")" "$MJ_DV_BRANCH" "$(mj_git_dirty)" "$n"
  if [ "$MJ_DV_START_HEAD" != "$MJ_DV_HEAD" ] && \
     mj_git merge-base --is-ancestor "$MJ_DV_START_HEAD" HEAD 2>/dev/null; then
    printf '\nCommits since the task started:\n\n'
    mj_git log --oneline --no-decorate "$MJ_DV_START_HEAD..HEAD" 2>/dev/null | sed 's/^/- /' \
      | mj_derive_bounded 8 "commit(s)"
  fi
  [ -n "$MJ_DV_QUESTIONS" ] && printf '\n%s open question(s) block acceptance; see `majordomus question list`.\n' \
    "$(mj_derive_nlines "$MJ_DV_QUESTIONS")"
  printf '\nDerived, not authored: `majordomus checkpoint --derive`.\n'
  return 0
}

# ---------------------------------------------------------------- session record body
# What a closed episode did, and how it turned out.
#
# The record's front matter is everything a machine can prove about the episode — head,
# branch, the commits, the files, the references. None of it says what the work was *for*
# or whether it worked. That half of the contract was optional and its only producer was a
# person typing into `majordomus session close`; the lifecycle closes an episode with
# `< /dev/null`, so on the automatic path — which is every path — the body was empty by
# construction. A record that names ninety-four commits and says nothing about them is not
# a record of the episode, it is a receipt.
#
# So the body is composed from two sources and is never empty:
#
#   * what the worker itself wrote down while the episode ran — the checkpoints, the
#     decisions, the questions it opened. That is the only material in the repository that
#     carries intent, and a model that leaves none gets a body that says so by name.
#   * what git and the ledger can prove — the commits with their subjects, the files, the
#     issues the plan moved, how the episode ended.
#
# Neither alone is enough. The derived half cannot say why; the authored half is absent
# whenever a session is cut short, which is the case that most needs a record. Composing
# both means the worst outcome is a body that is thin and honest about being thin, and
# never one that is blank.
#
# It asserts nothing it did not read. Where the worker wrote nothing, the section says so
# rather than inventing a summary, because a fabricated narrative in an append-only record
# is worse than an acknowledged gap.

# The sections a session body carries, from the policy, with the same contract the handover
# sections have: a required section with no writer is a configuration error named out loud.
MJ_DERIVE_SESSION_DEFAULT_SECTIONS="What Happened|Progress Notes|Decisions|Open Questions|Outcome"

# mj_derive_session_body <window-file> <outcome>
mj_derive_session_body() {
  MJ_DV_WINDOW="${1:-}"; MJ_DV_OUTCOME="${2:-}"
  mj_derive_gather
  local sec slug fn list ses_head

  # `mj_derive_gather` scopes to the *task*, which is right for a handover and wrong here.
  # A task outlives the episodes that work on it — this one had run for five days — so the
  # task's opening commit made the first draft of this section claim 1261 commits and 2339
  # changed files for an episode that had made none of them. An episode's record is scoped
  # to the episode, by the same `start_head` its own front matter records.
  ses_head="$(mj_ses start_head 2>/dev/null || true)"
  if [ -n "$ses_head" ] && [ "$ses_head" != NONE ]; then
    MJ_DV_START_HEAD="$ses_head"
    MJ_DV_CHANGED="$(mj_derive_changed "$MJ_DV_START_HEAD")"
  fi

  list="$(mj_ylist "$MJ_POL_FLAT" session.record_sections | sed '/^$/d')"
  [ -n "$list" ] || list="$(printf '%s' "$MJ_DERIVE_SESSION_DEFAULT_SECTIONS" | tr '|' '\n')"

  local IFS='
'
  for sec in $list; do
    [ -n "$sec" ] || continue
    slug="$(mj_derive_slug "$sec")"; fn="mj_derive_ses_$slug"
    # A session section falls back to the handover writer of the same name, so that
    # `Decisions` and `Open Questions` have one writer between the two records rather than
    # two that can disagree.
    command -v "$fn" >/dev/null 2>&1 || fn="mj_derive_sec_$slug"
    command -v "$fn" >/dev/null 2>&1 || mj_die "$MJ_EX_CONTRACT" \
      "derive: the policy requires the session section '$sec' and nothing derives it (add mj_derive_ses_$slug in lib/derive.sh)"
    printf '# %s\n\n' "$sec"; "$fn"; printf '\n'
  done
  unset IFS

  printf -- '---\n\nComposed by `majordomus session close` from what this episode wrote down and what git and the ledger can prove. Every statement above is one or the other; nothing here is a summary a model was asked for after the fact.\n'
  return 0
}

# The commits, with their subjects. The front matter lists the hashes, which is the
# checkable form and unreadable; this is the same list a person can act on.
mj_derive_ses_what_happened() {
  local n cap; cap="$(mj_derive_cap)"
  if [ "$MJ_DV_START_HEAD" != "$MJ_DV_HEAD" ] && \
     mj_git merge-base --is-ancestor "$MJ_DV_START_HEAD" HEAD 2>/dev/null; then
    n="$(mj_git rev-list --count "$MJ_DV_START_HEAD..HEAD" 2>/dev/null || printf 0)"
    printf '%s commit(s) on %s, %s..%s:\n\n' \
      "$n" "$MJ_DV_BRANCH" "$(printf '%.7s' "$MJ_DV_START_HEAD")" "$(printf '%.7s' "$MJ_DV_HEAD")"
    mj_git log --reverse --no-decorate --format='- %h %s' "$MJ_DV_START_HEAD..HEAD" 2>/dev/null \
      | mj_derive_bounded "$cap" "commit(s)"
  elif [ "$MJ_DV_START_HEAD" = "$MJ_DV_HEAD" ]; then
    printf 'No commit was made in this episode; %s did not move.\n' "$MJ_DV_BRANCH"
  else
    printf 'The history diverged during this episode: the commit it opened at (%s) is not an ancestor of %s, so the commit list cannot be computed and the front matter records `diverged`.\n' \
      "$(printf '%.7s' "$MJ_DV_START_HEAD")" "$(printf '%.7s' "$MJ_DV_HEAD")"
  fi
  n="$(mj_derive_nlines "$MJ_DV_CHANGED")"
  printf '\n%s file(s) changed since the episode opened.\n' "$n"
  [ -n "$MJ_DV_ISSUES" ] && printf 'Plan issues this episode moved: %s.\n' "$MJ_DV_ISSUES"
  return 0
}

# The worker's own progress notes. This is the half of the record no derivation can supply,
# and where a model wrote none the section says so with the command that would have.
mj_derive_ses_progress_notes() {
  local paths f n=0 cap body
  cap="$(mj_derive_cap)"
  # `mj_session_field` belongs to the session module, which is what calls this. The guard
  # is not defensive style: it keeps this file sourceable on its own, as the checkpoint and
  # briefing paths source it without session.sh.
  [ -n "${MJ_DV_WINDOW:-}" ] && [ -f "$MJ_DV_WINDOW" ] && command -v mj_session_field >/dev/null 2>&1 \
    && paths="$(mj_session_field "$MJ_DV_WINDOW" 'task.checkpoint' checkpoint_path)"
  if [ -z "${paths:-}" ]; then
    printf 'This episode recorded no checkpoint, so it left no note of its own about what it was doing or why. Everything above and below is derived.\n'
    printf '\nA worker that records progress with `majordomus checkpoint` puts its own words here; one that does not leaves this paragraph.\n'
    return 0
  fi
  local IFS='
'
  for f in $paths; do
    [ -n "$f" ] || continue
    n=$((n + 1))
    [ "$n" -gt "$cap" ] && continue
    # The path itself is local state and is not named in a shared record (ADR 0014); what
    # the worker wrote is not.
    [ -f "$MJ_ROOT/$f" ] || { printf -- '- (a checkpoint was recorded and its file is no longer present)\n'; continue; }
    body="$(mj_record_body "$MJ_ROOT/$f" | sed '/^$/d' | head -n 12)"
    [ -n "$body" ] || continue
    printf -- '- %s\n' "$(printf '%s' "$body" | head -n 1)"
    printf '%s\n' "$body" | tail -n +2 | sed 's/^/  /'
  done
  unset IFS
  [ "$n" -gt "$cap" ] && printf -- '- ... and %s more checkpoint(s)\n' "$((n - cap))"
  return 0
}

# The decisions this episode recorded — not the repository's recent ones. `mj_derive_sec_decisions`
# answers "what should the next worker know", which is a different question from "what did this
# episode decide", and reusing it here put five decisions from previous weeks under the heading.
mj_derive_ses_decisions() {
  local list n=0 d
  [ -n "${MJ_DV_WINDOW:-}" ] && [ -f "$MJ_DV_WINDOW" ] && command -v mj_session_field >/dev/null 2>&1 \
    && list="$(mj_session_field "$MJ_DV_WINDOW" 'decision.recorded' decision)"
  if [ -z "${list:-}" ]; then
    printf 'This episode recorded no decision. `majordomus decision` is what puts one here and in the durable record; nothing infers one from a diff.\n'
    return 0
  fi
  local IFS='
'
  for d in $list; do
    [ -n "$d" ] || continue
    n=$((n + 1)); printf -- '- %s\n' "$d"
  done
  unset IFS
  return 0
}

# How the episode ended, in the vocabulary the front matter uses, and what it left open.
mj_derive_ses_outcome() {
  case "${MJ_DV_OUTCOME:-}" in
    closed)      printf 'The episode ended on a close the provider reported: the window was closed, cleared or logged out of deliberately.\n' ;;
    interrupted) printf 'The episode ended without a deliberate close — the provider reported an end it does not classify as one. Work in progress at that moment is in the working tree, not in this record.\n' ;;
    *)           printf 'The episode ended; the provider reported no reason this tool classifies.\n' ;;
  esac
  printf '\nAt the close: on %s at %s, working tree %s.\n' \
    "$MJ_DV_BRANCH" "$(printf '%.7s' "$MJ_DV_HEAD")" "$(mj_git_dirty)"
  if [ "$MJ_DV_TASK_ID" = none ]; then
    printf 'No task was active, so nothing here was measured against a declared scope.\n'
  else
    printf 'Task %s (profile %s) was active.\n' "$MJ_DV_TASK_ID" "$MJ_DV_PROFILE"
    [ "$MJ_DV_N_HAND" != 0 ] && printf '%s continuation record(s) were written, which is where the next worker starts.\n' "$MJ_DV_N_HAND"
  fi
  local q; q="$(mj_derive_nlines "$MJ_DV_QUESTIONS")"
  if [ "$q" = 0 ]; then printf 'Nothing was left blocking acceptance.\n'
  else printf '%s open question(s) still block acceptance; `majordomus question list` names them.\n' "$q"; fi
  return 0
}

# ---------------------------------------------------------------- the opening briefing
# What the provider's start event writes to standard output, and therefore what the worker
# is holding before it has read anything or decided anything.
#
# It is deliberately not `majordomus context`. That command assembles the full briefing
# within its own budget and is the right thing to run once there is a question; this is what
# is true before there is one, and it is bounded by a smaller number for the same reason the
# always-loaded projection is: text that arrives in every episode whether or not it is
# needed is paid for in every episode.
#
# The three facts it carries are the three a worker cannot get wrong quietly — what episode
# it is in, what the last one left, and what is blocking acceptance. Everything else is a
# command away, and the last line names it.
#
# Absence is printed, not omitted. "No relevant handover" is a fact the next worker needs;
# silence is indistinguishable from a briefing that failed to run.
# The briefing the episode-start event writes. Its one argument is where the shared server
# stands, as `mj_capture_ensure_server` reported it; empty when nothing asked.
mj_derive_briefing() {
  local server="${1:-}" budget out
  budget="$(mj_pol session.briefing_budget_lines)"
  case "$budget" in ''|*[!0-9]*) budget="$(mj_pol context.always_loaded_budget_lines)" ;; esac
  case "$budget" in ''|*[!0-9]*) budget=60 ;; esac

  out="$(mj_derive_briefing_body "$server")"
  printf '%s\n' "$out" | head -n "$budget"
  local total; total="$(mj_derive_nlines "$out")"
  [ "$total" -gt "$budget" ] && printf '... %s more line(s) withheld by session.briefing_budget_lines; run `majordomus context` for the whole briefing.\n' "$((total - budget))"
  return 0
}

mj_derive_briefing_body() {
  local server="${1:-}" sid task_id outcome n
  sid="$(mj_open_session_id)"
  printf '## Majordomus — what this repository already knows\n\n'
  printf 'Episode %s, on %s at %s, working tree %s.\n' \
    "${sid:-(none open)}" "$(mj_git_branch)" "$(printf '%.7s' "$(mj_git_head)")" "$(mj_git_dirty)"

  # --- the task
  if mj_load_current; then
    task_id="$(mj_cur id)"; outcome="$(mj_cur outcome)"
    printf '\nActive task %s (%s, profile %s): %s\n' "$task_id" "$outcome" "$(mj_cur profile)" "$(mj_cur task)"
    printf 'Scope: %s\n' "$(mj_ylist "$MJ_CUR_FLAT" scope | tr '\n' ' ' | sed 's/ $//; s/ /, /g')"
  else
    printf '\nNo active task in this checkout. `majordomus start "<task>" --scope <paths>` opens one; work outside a task is permitted and records nothing that a task would.\n'
  fi

  # --- the shared server, when the start event ensured it: one line, so that a worker
  # knows before its first tool call whether the board it is about to be told to read exists
  if [ -n "$server" ]; then printf '\nShared server: %s\n' "$server"; fi

  # --- what blocks acceptance. First, because it is the only thing here that refuses a
  # command the worker is otherwise about to run.
  local q; q="$(mj_derive_questions)"
  if [ -n "$q" ]; then
    n="$(mj_derive_nlines "$q")"
    printf '\nBLOCKED: %s open question(s) on this branch; every one refuses `majordomus finish --outcome completed`.\n\n' "$n"
    printf '%s\n' "$q" | mj_derive_bounded 5 "question(s)"
  fi

  # --- the continuation record, with the label that says how far to trust it
  printf '\n'
  if mj_resolve_latest "$MJ_STATE_DIR/handovers" ""; then
    printf 'Handover %s (%s, %s, %s).\n' "${MJ_RES_PATH#"$MJ_ROOT/"}" "$MJ_RES_MATCH" \
      "$(mj_git_label "$MJ_RES_HEAD" "$MJ_RES_BRANCH")" \
      "$(mj_age_human "$(mj_age_minutes "$MJ_RES_CREATED" || true)")"
    case "$(mj_git_label "$MJ_RES_HEAD" "$MJ_RES_BRANCH")" in
      diverged|different_context)
        printf 'Its commit is not in this history. Trust git over anything it says.\n' ;;
    esac
    # The one section a resuming worker acts on. The rest of the record is at the path
    # above; quoting all of it here would put the whole document in every episode.
    local next; next="$(mj_derive_section "$MJ_RES_PATH" "Next Action")"
    if [ -n "$next" ]; then printf '\nIts Next Action:\n\n'; printf '%s\n' "$next" | mj_derive_bounded 12 "line(s)"; fi
  else
    printf 'No relevant handover for this worktree and branch. That is an answer, not a gap: a record from another branch is never offered, because a briefing that is quietly about somebody else is worse than none.\n'
  fi

  printf '\nRun `majordomus context` for the full briefing, `majordomus check` before claiming anything is done.\n'
  return 0
}

# One level-one section of a record, body only. Used to quote the part of a handover a
# resuming worker acts on without copying the document into every episode.
# Blank lines inside the section are kept and blank lines around it are not. The first
# version dropped every blank line, which ran a section's paragraphs together into one block
# — in the one part of a record a resuming worker is told to act on.
mj_derive_section() {
  awk -v want="$2" '
    /^# / { cur = substr($0, 3); sub(/[ \t]+$/, "", cur); on = (cur == want); next }
    on { hold[++n] = $0; if (NF) last = n; if (NF && !first) first = n }
    END { for (i = first; i && i <= last; i++) print hold[i] }' "$1"
}

#!/usr/bin/env bash
# sourced by bin/majordomus; guard against re-sourcing
[ -n "${MJ_LIB_recover:-}" ] && return 0 || MJ_LIB_recover=1
# recover — bring this checkout's record stores back to a state the contract describes,
# by evidence, exactly once, without inventing anything.
#
# The stores degrade in three ways and nothing owned any of them.
#
#   A stranded episode. An open episode lives in state/sessions-open/<provider session>.yaml
#   until an end event closes it. A provider that is killed, crashes, or is disconnected
#   sends no end event, and the file stays. It is then open for ever: `session start` with
#   that key is refused, `continuity` reports an episode nobody is in, and the ledger lines
#   of whoever *is* working carry no relation to it. Measured in this repository's primary
#   checkout on 2026-09-11: five open episodes, of which two had not stamped a ledger line
#   since the previous evening and three were live.
#
#   A duplicate record. `session close` publishes through `mj_publish_record`, which gives
#   every file a unique name — so a second end event for one episode wrote a second *file*
#   rather than colliding, and nothing refused it. Episode s-20260909152316-024f has four
#   records: one at 21:41:36 and three more the next morning, 13 and 15 seconds apart. The
#   close path has since grown a guard, and that guard stops the fifth; it does nothing
#   about the four that exist.
#
#   A stray file. `mj_publish_record` creates its temp with `mktemp "$dir/.tmp.XXXXXX"` and
#   removes it after the hard link; a process that dies between the two leaves it behind, in
#   a tracked section, looking like a record. `scripts/generate-site-data` stages into
#   `$ROOT/.mj-stage.XXXXXX`. Neither is a record and neither has an owner.
#
# What this command will not do. It never deletes a file because a prompt said it was
# rubbish: every subject classifies each candidate by reading it, prints the evidence that
# decided the verdict before acting on it, and leaves anything it could not read alone —
# the shape `project.destructive-sweeps-fail-closed` requires of any sweep that selects by
# a measured predicate, and the first executable instance of that rule in this repository.
# It never fabricates history: a recovered episode's record carries what the open record and
# the ledger can prove and states the rest as absent, because the working tree at recovery
# time belongs to whoever is working now.
#
# Why this is not a subcommand of `migrate`. `migrate` is the one-time move of a repository
# off the pre-.ai layout: it refuses unless .majordomus/policy.yaml is present, takes a
# byte-for-byte backup, and is done for ever afterwards. Recovery is recurring, idempotent
# maintenance of a store that is already in the right place. Putting one inside the other
# would give `migrate` two contracts and one exit code space, and would leave `migrate
# --dry-run` meaning two different things depending on which half of it you had reached.

# shellcheck source=session.sh
# shellcheck disable=SC1091
. "$MJ_LIB_DIR/session.sh"

# The staleness threshold is `session.stranded_after`, and it is read with no default
# beside it. Deliberately generous: closing a live episode is the expensive mistake — the
# worker inside it loses the boundary of the work it is doing — and leaving a dead one open
# for another hour is the cheap one, so the declared value errs entirely in the cheap
# direction and `--older-than` is there for anybody with better evidence than a clock.
#
# The first draft of this file read the key and fell back to a constant written here when
# the policy had none. `majordomus doctor` refused it in one line — "read by lib/ but absent
# from share/skeleton/policy.yaml" — and it was right twice over: a reader that carries its
# own default is a second place the number lives, and a repository whose policy says 6h
# would still have been recovered against 12h by any installation that had not been
# updated. The key is declared in the schema, the skeleton and this repository's policy, and
# read with mj_pol_req, which fails closed.

mj_cmd_recover() {
  local sub="${1:-status}"
  case "$sub" in
    --help|-h|help) mj_recover_usage; return 0 ;;
    status|episodes|records|orphans|all) shift || true ;;
    --*) sub=status ;;
    *) mj_die "$MJ_EX_USAGE" "recover: unknown subject '$sub' (see: majordomus recover --help)" ;;
  esac
  mj_require_installed
  # The threshold is a policy value, so the policy has to be loaded before it is read. The
  # first draft did not, mj_pol returned the empty string for a key that was declared, and
  # the fallback beside it hid that for as long as there was a fallback. There is not one
  # now, which is how it was found.
  mj_load_policy || mj_die "$MJ_EX_CONTRACT" "policy does not parse (run: majordomus doctor)"

  local check=0 age=""
  while [ $# -gt 0 ]; do case "$1" in
    --check|--dry-run) check=1; shift ;;
    --older-than) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--older-than needs a value"; age="$2"; shift 2 ;;
    --older-than=*) age="${1#--older-than=}"; shift ;;
    --help|-h) mj_recover_usage; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "recover: unknown option $1" ;;
  esac; done

  # The threshold is resolved once, here, so that every subject and the status report
  # measure against the same number and a refusal names the value the caller actually got.
  # mj_pol_req returns non-zero rather than exiting, because inside a command substitution
  # mj_die can only kill the subshell; the caller is what decides, and a command decides to
  # die. A recovery that ran against an empty threshold would treat every episode as
  # stranded, which is the one failure this command must not have.
  if [ -z "$age" ]; then
    age="$(mj_pol_req session.stranded_after)" \
      || mj_die "$MJ_EX_CONTRACT" "recover: the policy declares no session.stranded_after, and nothing here invents one (run: majordomus doctor)"
  fi
  MJ_RECOVER_AGE_SECS="$(mj_duration_secs "$age")" \
    || mj_die "$MJ_EX_USAGE" "recover: '$age' is not a duration (use 90s, 30m, 12h)"
  MJ_RECOVER_AGE="$age"

  case "$sub" in
    status)   MJ_RECOVER_CHECK=1; mj_recover_all; return 0 ;;
    all)      MJ_RECOVER_CHECK="$check"; mj_recover_all; return 0 ;;
    episodes) MJ_RECOVER_CHECK="$check"; mj_recover_episodes ;;
    records)  MJ_RECOVER_CHECK="$check"; mj_recover_records ;;
    orphans)  MJ_RECOVER_CHECK="$check"; mj_recover_orphans ;;
  esac
  mj_recover_epilogue
}

# The order is a correctness property, not a presentation choice, and the real store proved
# it. In the primary checkout of this repository on 2026-09-11, episode s-20260910200453-afbb
# was open *and* stranded, and .ai/repo/sessions/.tmp.5bcIMW held the only copy of its closed
# record — a publish that wrote the temp, died before the hard link, and never tore the open
# record down. Run episodes first and that episode gets a freshly synthesised record with
# empty commits and empty changed_files; orphans then finds the temp's episode "already
# published" and removes the temp, discarding the complete record in favour of the minimal
# one. So orphans runs first: a record that already exists is always better evidence than one
# reconstructed after the fact, and `recover episodes` then finds it and only finishes the
# teardown. Records folds last, because the two subjects before it can each publish.
mj_recover_all() {
  mj_recover_orphans;  printf '\n'
  mj_recover_episodes; printf '\n'
  mj_recover_records
  mj_recover_epilogue
}

# What a run leaves behind, in one line. `status` is the read-only spelling of the same
# report, so a person is never told to run something to find out whether to run it.
mj_recover_epilogue() {
  printf '\n'
  # Having nothing to do is the same sentence whichever mode asked, because it is the same
  # fact. "recovered: 0 action(s)" reads like a run that failed to do something it meant to.
  if [ "${MJ_RECOVER_ACTS:-0}" = 0 ]; then
    printf 'nothing to recover; the stores are as the contract describes them\n'
  elif [ "$MJ_RECOVER_CHECK" = 1 ]; then
    printf 'check: %s action(s) would be taken and nothing was written (run: majordomus recover all)\n' "$MJ_RECOVER_ACTS"
  else
    printf 'recovered: %s action(s); re-running this command now takes none\n' "$MJ_RECOVER_ACTS"
  fi
  # A candidate nobody could measure is the finding this whole command is shaped around; it
  # is reported whether or not anything was done, and never folded into the action count.
  [ "${MJ_RECOVER_SKIPPED:-0}" = 0 ] || printf 'skipped %s candidate(s) whose evidence could not be read; none of them was acted on\n' "$MJ_RECOVER_SKIPPED"
  return 0
}

mj_recover_usage() {
  cat <<H
usage: majordomus recover [<subject>] [--check] [--older-than <duration>]

  status                    what is degraded here and what recovery would do  (read-only)
  orphans                   classify the stray files of the record stores
  episodes                  close every open episode whose client is demonstrably gone
  records                   fold duplicate records of one episode into one canonical record
  all                       every subject above, in that order — and the order matters: a
                            record an interrupted publish left in a temp is better evidence
                            than one synthesised for the same episode a moment later

  --check                   print the plan and the evidence behind it; write nothing
  --older-than <duration>   the staleness threshold (90s, 30m, 12h): how long an episode
                            may be silent, and how old a stray file must be, before either
                            is a candidate. Default: the policy's session.stranded_after

  Recovery is by evidence and never by assumption. An episode is stranded when its last
  sign of life — the later of its own started_at and the newest ledger line it stamped — is
  older than the threshold. This process's own episode is never a candidate, whatever its
  age, and neither is one belonging to another worktree. An episode whose evidence cannot
  be read is skipped and counted, never closed: a predicate that cannot measure its input
  is false (project.destructive-sweeps-fail-closed).

  A recovered episode is closed with outcome: interrupted, which is what it was, and its
  record carries what the open record and the ledger prove. It claims no commits and no
  changed files: the working tree at recovery time is whoever is working now, not the
  episode that stopped. The reason is a ledger event, session.recovered.

  A stray file is judged by its age before its content. A publish temp seconds old belongs
  to a session close running right now, and a staging directory minutes old to a derive
  running right now; both are reported as live and neither is touched. A file whose age
  this platform cannot read is skipped and counted.

  Every subject is idempotent. Running it twice takes the actions once; running it after a
  crash mid-run takes the ones that did not happen. Nothing is deleted for being unknown:
  a file this command cannot classify is reported and left where it is.
H
}

# ---------------------------------------------------------------- evidence
# The last moment each episode is known to have been alive, as "<session id> <epoch>", read
# from the ledger in one pass. One pass rather than one per episode: this is the ledger of a
# repository, it is thousands of lines, and a grep per candidate is how a report that should
# be instant becomes a wait.
#
# An episode's ledger lines are the ones it stamped with its own id — the same selection
# `mj_session_window` makes, and for the same reason: a time range cannot tell two workers
# in one repository apart.
mj_recover_ledger_seen() {
  local led="$MJ_STATE_DIR/ledger.jsonl"
  [ -f "$led" ] || return 0
  # session.recovered is skipped, and this is not a detail. The envelope stamps every ledger
  # line with the episode the writing process resolves to, which — when nothing names one —
  # is whatever the pointer happens to be aimed at, and that can be the very episode being
  # recovered. Counting it would make recovery refresh the evidence it judges by: publishing
  # a rescued record for a stranded episode moved that episode's last sign of life to *now*,
  # and the episodes pass a second later then found it young and left it open for ever.
  # Measured while writing test/cases/135; a recovery is not a sign of life.
  awk "$MJ_LEDGER_FIELD_AWK"'
       { e = mjfield($0, "event"); if (e == "" || e == "session.recovered") next
         s = mjfield($0, "session"); if (s == "") next
         t = mjfield($0, "ts");      if (t == "") next
         if (!(s in last) || t > last[s]) last[s] = t }
       END { for (s in last) print s "\t" last[s] }' "$led"
  return 0
}

# ---------------------------------------------------------------- episodes
mj_recover_episodes() {
  printf 'episodes — open episodes in %s\n' "$(mj_rel "$(mj_session_open_dir)")"
  local seen mine line key f sid started last src now_e last_e age n=0 acted=0
  seen="$(mktemp "${TMPDIR:-/tmp}/mj.rl.XXXXXX")"
  mj_recover_ledger_seen > "$seen"
  now_e="$(mj_epoch "$(mj_now)" || true)"

  # This process's own episode, by the same resolution every other command uses. It is the
  # one episode whose liveness needs no measurement — something is running inside it right
  # now — and it is excluded before any predicate is evaluated rather than after, so no
  # arithmetic mistake can ever reach it.
  mine=""
  local myf; myf="$(mj_session_here_file)"
  [ -f "$myf" ] && mine="$(sed -n 's/^session_id: //p' "$myf" | head -n 1)"

  while IFS= read -r line; do
    [ -n "$line" ] || continue
    key="${line%%|*}"; f="${line#*|}"
    n=$((n + 1))
    sid="$(sed -n 's/^session_id: //p' "$f" | head -n 1)"
    started="$(sed -n 's/^started_at: //p' "$f" | head -n 1)"
    local wt; wt="$(sed -n 's/^worktree: //p' "$f" | head -n 1)"

    # An episode with no identity is not an episode this command can close: the record it
    # would write has no session_id, and the ledger event would name nothing. Reported,
    # counted as unreadable, left alone.
    if [ -z "$sid" ]; then
      mj_warn recover "$(mj_rel "$f")" "carries no session_id; it is not a record this version can close" "cat $(mj_rel "$f")"
      MJ_RECOVER_SKIPPED=$((${MJ_RECOVER_SKIPPED:-0} + 1)); continue
    fi

    # The last sign of life: the later of the open record's own started_at and the newest
    # ledger line this episode stamped. Both are read; neither is assumed.
    last="$(awk -F'\t' -v s="$sid" '$1 == s { print $2 }' "$seen")"
    src=ledger
    if [ -z "$last" ]; then last="$started"; src=started_at; fi
    # Not `[ -n "$last" ] && last_e="$(mj_epoch "$last")"`. bin/majordomus runs under
    # `set -eu`, and an assignment that follows the final `&&` of a list is *not* exempt
    # from errexit: mj_epoch returns non-zero on a string neither date can parse, so the
    # whole command died on the first unreadable candidate having printed one header line.
    # The guard this file exists to demonstrate was the thing that killed it.
    last_e=""
    if [ -n "$last" ]; then last_e="$(mj_epoch "$last" || true)"; fi

    # The guard the rule is about. An age computed from an empty string is not a large age,
    # it is not an age; the candidate is excluded and counted, and the sweep says so.
    if [ -z "$last_e" ] || [ -z "$now_e" ]; then
      mj_warn recover "$sid" "last sign of life is '${last:-<absent>}', which this platform cannot read as a timestamp; not a candidate" "sed -n 's/^started_at: //p' $(mj_rel "$f")"
      MJ_RECOVER_SKIPPED=$((${MJ_RECOVER_SKIPPED:-0} + 1)); continue
    fi
    age=$((now_e - last_e))

    # The measurement, before the verdict and before any action — printed for every
    # candidate, including the ones nothing will happen to, so that a wrong predicate is
    # visible in the output rather than in its consequences.
    printf '  %s  key=%s  last seen %s (%s, %s)\n' "$sid" "$key" "$last" "$src" "$(mj_age_human $((age / 60)))"

    # Two independent guards stand between a live episode and this loop, and neither of
    # them is the other's backstop. This is the first: the episode this process resolves to.
    # It resolves through the pointer when nothing names one, and the pointer is a guess
    # between two open episodes — `mj_session_here_file` says so itself. The second guard is
    # the threshold below, and it is what makes the guess safe: an episode somebody is
    # actually working in has stamped a ledger line recently, so it is young whichever way
    # the pointer happens to be aimed.
    if [ "$sid" = "$mine" ]; then
      printf '    live: this process is inside it; never a candidate\n'; continue
    fi
    if [ -n "$wt" ] && [ "$wt" != "$MJ_ROOT" ]; then
      printf '    foreign: opened in %s; close it there, not here\n' "$wt"; continue
    fi
    if [ "$age" -lt "$MJ_RECOVER_AGE_SECS" ]; then
      printf '    live: %s, under the %s threshold\n' "$(mj_age_human $((age / 60)))" "$MJ_RECOVER_AGE"
      continue
    fi

    printf '    stranded: %s, over the %s threshold; close with outcome interrupted\n' \
      "$(mj_age_human $((age / 60)))" "$MJ_RECOVER_AGE"
    MJ_RECOVER_ACTS=$((${MJ_RECOVER_ACTS:-0} + 1))
    [ "$MJ_RECOVER_CHECK" = 1 ] && continue
    mj_recover_close_episode "$f" "$sid" "$started" "$last" "$src" "$age" && acted=$((acted + 1))
  done <<EOF
$(mj_session_open_list)
EOF
  rm -f "$seen"
  [ "$n" = 0 ] && printf '  (none open)\n'
  return 0
}

# Close one stranded episode into a record, exactly once.
#
# The record is composed here rather than by `session close`, and the difference is the
# whole point. `session close` records the commits between the episode's start_head and
# HEAD, and the working tree's changed files, because the worker closing an episode is the
# worker who made them. Nobody is closing this one. The tree at this moment belongs to
# whoever is working now, and stamping it onto an episode that stopped yesterday would be a
# record that looks exactly like a true one. So both lists are written empty — `[]`, the
# explicit empty this section already distinguishes from absent — and everything else comes
# from the open record and from the ledger window, which are the two things that can be
# proved about an episode after the fact.
mj_recover_close_episode() {
  local f="$1" sid="$2" started="$3" last="$4" src="$5" age="$6"
  local closed_at rec win final existing

  mj_session_store_ready
  # Exactly once, by identity. A record for this episode already existing is the normal
  # outcome of a re-run and of a crash between the publish and the removal below; it is not
  # an error, and the correct move is to keep the record that exists and finish the teardown.
  existing="$(grep -rl "^session_id: $sid\$" "$(mj_session_store)" 2>/dev/null | head -n 1 || true)"
  if [ -n "$existing" ]; then
    printf '    record exists at %s; none written, the open record is removed\n' "$(mj_rel "$existing")"
    final="$existing"
  else
    closed_at="$(mj_now)"
    win="$(mktemp "${TMPDIR:-/tmp}/mj.rw.XXXXXX")"
    mj_session_window "$sid" > "$win"
    rec="$(mktemp "${TMPDIR:-/tmp}/mj.rr.XXXXXX")"
    {
      printf -- '---\nschema: session/v1\nkind: session\ncreated_at: %s\ntask_id: %s\nprofile: %s\n' \
        "$closed_at" none none
      printf 'repository_id: %s\nworktree_id: %s\nbranch: %s\n' \
        "$(mj_repository_id)" "$(mj_worktree_id)" "$(sed -n 's/^branch: //p' "$f" | head -n 1)"
      # head and working_tree describe the close everywhere else in this section. There was
      # no close: nobody was here to make one, and the head this recovery happens to stand
      # on is not a fact about the episode. The two keys are omitted rather than filled in,
      # which is the difference between a record that is short and one that is wrong.
      printf 'changed_files: []\n'
      printf 'session_id: %s\nstarted_at: %s\nclosed_at: %s\noutcome: interrupted\n' \
        "$sid" "$started" "$closed_at"
      printf 'title: "Recovered episode %s"\n' "$sid"
      local worker; worker="$(sed -n 's/^worker: //p' "$f" | head -n 1)"
      [ -n "$worker" ] && printf 'worker: %s\n' "$worker"
      printf 'start_head: %s\nstart_working_tree: %s\n' \
        "$(sed -n 's/^start_head: //p' "$f" | head -n 1)" \
        "$(sed -n 's/^start_working_tree: //p' "$f" | head -n 1)"
      printf 'commits: []\n'
      mj_session_refs "$win"
      printf -- '---\n'
      # The prose half says what happened and how it was decided, in the reader's language.
      # A record whose front matter is short for a reason should say the reason where the
      # person reading it months later will be standing.
      # shellcheck disable=SC2016  # the backticks are Markdown, not a substitution
      printf '\nThis episode was closed by `majordomus recover`, not by the worker who opened it.\n'
      printf 'No end event ever arrived: the provider was killed, crashed, or lost its connection.\n\n'
      printf -- '- last sign of life: %s (from the %s)\n' "$last" "$src"
      printf -- '- age at recovery: %s, over the %s threshold\n' "$(mj_age_human $((age / 60)))" "$MJ_RECOVER_AGE"
      printf -- '- commits and changed_files are empty because nothing can prove them after the fact:\n'
      printf '  the working tree at recovery time belongs to whoever is working now.\n'
      printf -- '- everything above the body comes from the open record and from the ledger lines\n'
      printf '  this episode stamped with its own id.\n'
    } > "$rec"
    final="$(mj_publish_record "$(mj_session_store)" "$sid" "$rec")" \
      || { rm -f "$rec" "$win"; mj_err "recover: could not create a unique session file for $sid"; return 1; }
    rm -f "$rec" "$win"
    printf '    wrote %s\n' "$(mj_rel "$final")"
  fi

  # The reason, as a ledger event. The envelope's own `session` stamp names the episode
  # doing the recovering, which is not this one, so the recovered episode is named in the
  # payload — the event would otherwise record that somebody recovered something.
  mj_ledger_append session.recovered \
    "\"session_id\":\"$(mj_json_esc "$sid")\",\"reason\":\"no end event; last sign of life $(mj_json_esc "$last") from the $src, $((age / 60))m before recovery\",\"session_path\":\"$(mj_json_esc "${final#"$MJ_ROOT/"}")\""

  # The teardown, in the order that survives an interruption: the record exists before the
  # open file goes, so a crash between them loses nothing and the re-run finds the record
  # and finishes the job.
  rm -f "$f"
  local ptr; ptr="$(mj_session_pointer)"
  if [ -L "$ptr" ] && [ ! -e "$ptr" ]; then rm -f "$ptr"; fi
  return 0
}

# ---------------------------------------------------------------- records
# One episode, one record. Where several exist, the union of what they prove becomes one
# canonical record and the superseded files go — but only after the union is written, and
# only when nothing in them conflicts.
#
# This is the one place in the tool that edits a record after it was published, and
# `majordomus.session-records` says nothing does: "written once ... and nothing edits one
# afterwards." The tension is real and is resolved here in favour of the union, for three
# reasons, and it is written down rather than left for a reader to notice.
#
# The alternative that keeps the letter of the rule is to pick one of the four and delete
# the rest, which destroys provable facts — the 21:41 record of s-20260909152316-024f
# carries 94 commits and 3 changed files, the 10:39 ones carry 296 and 122, and no single
# file among them is the episode. Writing a fifth file instead would edit nothing and change
# nothing else: it is still one record replacing four, with a different name.
#
# The edit is bounded and it is recorded. Only the two list fields move, and only ever by
# gaining entries the other records of the same episode already prove; every scalar — the
# identity, the times, the outcome — comes from the canonical record untouched; a
# disagreement on any of them refuses the whole fold. The record then carries a `## Recovery`
# section naming how many there were and which files were superseded, so the edit is visible
# in the object itself and not only in the ledger.
#
# And it is repair, not practice. `mj_session_close` has grown the guard that stops a second
# record ever being published for one episode, so this path exists for damage that already
# happened and cannot be reached by normal use. The rule wants one record per episode; this
# is the only way to get there from four without throwing two of them away.
mj_recover_records() {
  local store sid ids n=0
  store="$(mj_session_store)"
  printf 'records — closed episodes in %s\n' "$(mj_rel "$store")"
  [ -d "$store" ] || { printf '  (no store)\n'; return 0; }
  ids="$(grep -h '^session_id: ' "$store"/*.md 2>/dev/null | sed 's/^session_id: //' | LC_ALL=C sort | uniq -d)"
  if [ -z "$ids" ]; then printf '  no episode has more than one record\n'; return 0; fi
  while IFS= read -r sid; do
    [ -n "$sid" ] || continue
    n=$((n + 1))
    mj_recover_one_record "$store" "$sid"
  done <<EOF
$ids
EOF
  return 0
}

mj_recover_one_record() {
  local store="$1" sid="$2" files keep f n started conflict=""
  files="$(grep -l "^session_id: $sid\$" "$store"/*.md 2>/dev/null | LC_ALL=C sort)"
  n="$(printf '%s\n' "$files" | grep -c . || true)"
  printf '  %s — %s records\n' "$sid" "$n"

  # The canonical record is the oldest by created_at: the first close is the one that
  # happened when the episode ended, and every later one is a repeat of an end event the
  # provider sent again. Oldest by the field, not by the file name — the name carries the
  # publish time, and a record restored from a backup would sort wrongly by it.
  keep=""
  local best="" ts
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    ts="$(sed -n 's/^created_at: //p' "$f" | head -n 1)"
    if [ -z "$ts" ]; then
      mj_warn recover "$(mj_rel "$f")" "carries no created_at; the canonical record cannot be chosen while it is here" "head -n 6 $(mj_rel "$f")"
      MJ_RECOVER_SKIPPED=$((${MJ_RECOVER_SKIPPED:-0} + 1)); return 0
    fi
    printf '    %s  created_at %s  %s changed_files  %s commits\n' \
      "$(basename "$f")" "$ts" "$(mj_recover_count_list "$f" changed_files)" "$(mj_recover_count_list "$f" commits)"
    if [ -z "$best" ] || [ "$ts" \< "$best" ]; then best="$ts"; keep="$f"; fi
  done <<EOF
$files
EOF

  # A conflict is a field on which two records of one episode disagree about something
  # neither of them could have got wrong: the identity and the times of the episode itself.
  # Reported, never resolved by picking — an episode whose records disagree about when it
  # started is a defect upstream of this command.
  started="$(sed -n 's/^started_at: //p' "$keep" | head -n 1)"
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    [ "$f" = "$keep" ] && continue
    local s; s="$(sed -n 's/^started_at: //p' "$f" | head -n 1)"
    [ "$s" = "$started" ] || conflict="$conflict$(basename "$f"): started_at $s, not $started"$'\n'
  done <<EOF
$files
EOF
  if [ -n "$conflict" ]; then
    printf '%s' "$conflict" | while IFS= read -r line; do
      [ -n "$line" ] && mj_fail recover "$sid" "records disagree — $line" "grep -l '^session_id: $sid\$' $(mj_rel "$store")/*.md"
    done
    printf '    conflict: nothing folded, nothing removed\n'
    MJ_RECOVER_SKIPPED=$((${MJ_RECOVER_SKIPPED:-0} + 1))
    return 0
  fi

  # The union of what they prove. Only the list fields are unioned: they are the ones a
  # later close can legitimately have more of, because it ran later and the ledger had
  # grown. Scalars are the episode's identity and come from the canonical record.
  printf '    canonical: %s (oldest created_at %s)\n' "$(basename "$keep")" "$best"
  printf '    union: %s changed_files, %s commits across all %s\n' \
    "$(mj_recover_union_list_uniq "$files" changed_files | grep -c . || true)" \
    "$(mj_recover_union_list_uniq "$files" commits | grep -c . || true)" "$n"
  local superseded; superseded="$(printf '%s\n' "$files" | grep -v "^$keep\$" || true)"
  # Repository-relative, never the absolute path the glob produced: a finding that names a
  # disk is a finding nobody else can act on (project.no-machine-paths).
  local l
  while IFS= read -r l; do
    [ -n "$l" ] && printf '    supersede: %s\n' "$(mj_rel "$l")"
  done <<EOF
$superseded
EOF
  MJ_RECOVER_ACTS=$((${MJ_RECOVER_ACTS:-0} + 1))
  [ "$MJ_RECOVER_CHECK" = 1 ] && return 0

  mj_recover_fold_record "$keep" "$files" changed_files
  mj_recover_fold_record "$keep" "$files" commits
  # The provenance, in the record itself: a reader who finds one record where four files
  # were must be able to see that from the record and not only from the ledger.
  #
  # Written once. A run interrupted between the fold and the removals below leaves the
  # canonical record folded and the superseded files still there, and the re-run that
  # finishes the job would otherwise append a second section — with a different count, since
  # one of the four is now the folded one. The folds themselves are idempotent because the
  # union de-duplicates; this is the part that would not have been.
  if ! grep -q '^## Recovery$' "$keep"; then
  {
    printf '\n## Recovery\n\n'
    # shellcheck disable=SC2016  # the backticks are Markdown, not a substitution
    printf 'This episode had %s closed records. `majordomus recover records` folded them into\n' "$n"
    printf 'this one: the oldest by created_at, with the union of the changed_files and commits\n'
    printf 'the others proved. The superseded files were:\n\n'
    printf '%s\n' "$superseded" | sed '/^$/d; s|^.*/|- |'
    printf '\nNo two of them disagreed on the identity or the times of the episode; a disagreement\n'
    printf 'would have been reported and nothing would have been folded.\n'
  } >> "$keep"
  fi
  printf '%s\n' "$superseded" | while IFS= read -r f; do
    [ -n "$f" ] || continue
    if mj_git ls-files --error-unmatch -- "$(mj_rel "$f")" >/dev/null 2>&1; then mj_git rm -q -- "$(mj_rel "$f")"
    else rm -f "$f"; fi
  done
  mj_ledger_append session.recovered \
    "\"session_id\":\"$(mj_json_esc "$sid")\",\"reason\":\"$n duplicate records folded into one; the union of their lists preserved\",\"session_path\":\"$(mj_json_esc "${keep#"$MJ_ROOT/"}")\""
  printf '    folded into %s\n' "$(mj_rel "$keep")"
  return 0
}

# How many entries a record's block list has. Absent and empty are different facts here as
# everywhere else: an absent key counts 0 and prints as 0, and so does `[]`.
mj_recover_count_list() {
  awk -v k="$2" '$0 == k ":" { f = 1; next } f && /^  - / { n++; next } f { exit } END { print n + 0 }' "$1"
}
# Every value of one block list across a set of files, de-duplicated, in the order the
# canonical record had them and then the order the others added them.
#
# The de-duplication is here and not at the call site. It was at the call site once, in the
# fold, and the *report* therefore counted six changed files where the fold wrote four: a
# plan that does not describe what the action will do is worse than no plan, and this
# command's whole contract is that the plan is the action.
mj_recover_union_list() {
  local files="$1" key="$2" f
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    awk -v k="$key" '$0 == k ":" { f = 1; next } f && /^  - / { sub(/^  - /, ""); print; next } f { exit }' "$f"
  done <<EOF
$files
EOF
}
# The same list, once each, first appearance kept: the order a worker moved through is
# information, and sorting it away is what mj_session_field refuses to do for the same
# reason.
mj_recover_union_list_uniq() { mj_recover_union_list "$@" | awk '!seen[$0]++'; }

# Replace one block list in the canonical record with the union across every record of the
# episode. Written through a temp and renamed, like every other record mutation in this
# tool, so an interrupted fold leaves the original.
#
# The union reaches awk as a file and not as `-v u="$union"`. A multi-line value in an awk
# variable is a GNU extension; the awk on this platform answered "newline in string" and
# every fold failed. The file is read once, where the key appears, and closed.
mj_recover_fold_record() {
  local keep="$1" files="$2" key="$3" tmp uni
  uni="$(mktemp "${TMPDIR:-/tmp}/mj.ru.XXXXXX")"
  mj_recover_union_list_uniq "$files" "$key" > "$uni"
  tmp="$keep.mj-tmp"
  awk -v k="$key" -v u="$uni" '
    $0 == k ":" || $0 == k ": []" {
      n = 0
      while ((getline line < u) > 0) if (line != "") { if (n == 0) print k ":"; print "  - " line; n++ }
      close(u)
      # An absent key and an empty list are different facts in this section, and a fold that
      # found nothing to union must write the empty one rather than a bare key.
      if (n == 0) print k ": []"
      skip = 1; next
    }
    skip && /^  - / { next }
    { skip = 0; print }' "$keep" > "$tmp"
  rm -f "$uni"
  mv "$tmp" "$keep"
  return 0
}

# ---------------------------------------------------------------- orphans
# Every stray file of the record stores, classified by what is in it. Five verdicts:
#
#   valid       a record the contract describes; nothing to do
#   live        younger than the staleness threshold: a `session close` or a `scripts/derive`
#               running right now owns it. Measured, reported, never touched
#   incomplete  a publish that died before its hard link: the content is a record, and no
#               record of that episode exists. Recoverable — it is published, not deleted
#   orphan      a temp whose work completed, or one with nothing in it. Removable
#   foreign     not written by this tool at all. Never touched; reported with what it is
#   legacy      a store an older version wrote and this one does not. Reported; a store with
#               a living writer is not legacy, whatever its age. Nothing in this repository
#               earns it: archive/ and completed/ both still have a writer, which is why
#               they are reported as `live` with the writer named
# How old a path is, in seconds, or nothing when it cannot be measured.
#
# The guard every stray-file verdict passes through, and it is not decoration. A publish
# temp a few milliseconds old belongs to a `session close` running right now; a
# `.mj-stage.XXXXXX` a few minutes old belongs to a `scripts/derive` running right now — on
# this machine, four of them ran at once while this file was being written, and one of them
# was this checkout's. Removing either is taking somebody's instrument out of their hands
# (project.reclaim-only-what-you-own), and "the run that made it did not finish" is a claim,
# not a measurement, until something reads the clock.
#
# `stat` is spelled twice because BSD and GNU disagree, and the ORDER is load-bearing.
# GNU first. `stat -f` means two different things: on BSD it is "this format string", on GNU
# it is "report the FILE SYSTEM, not the file". So `stat -f %m` asked GNU for a filesystem
# field that does not exist, and on Linux every age read failed — the whole subject reported
# "its age cannot be read on this platform" for all seven candidates and did nothing at all.
# CI caught it on the first run; macOS never would have, because BSD answers `-f %m`
# correctly. `stat -c` is unambiguous: GNU accepts it, BSD rejects the option and falls
# through. The guard behaved perfectly while this was wrong — nothing was deleted, every
# candidate was skipped and counted — which is the whole point of
# project.destructive-sweeps-fail-closed: a measurement that breaks must cost an action, not
# take the wrong one. The test now asserts the ages are read, so neither platform can regress
# to doing nothing quietly.
mj_recover_age_secs() {
  local m now
  m="$(stat -c %Y "$1" 2>/dev/null || stat -f %m "$1" 2>/dev/null || true)"
  case "$m" in ''|*[!0-9]*) return 1 ;; esac
  now="$(date +%s 2>/dev/null || true)"
  case "$now" in ''|*[!0-9]*) return 1 ;; esac
  printf '%s' $((now - m))
}

# Is this stray path old enough to act on? Prints the measurement either way, because the
# rule requires the value that decided a verdict to be visible before the verdict acts.
# 0 act · 1 too young · 2 unmeasurable (the caller counts it as skipped).
mj_recover_stray_ready() {
  local path="$1" age
  age="$(mj_recover_age_secs "$path")" || {
    mj_warn recover "$(mj_rel "$path")" "its age cannot be read on this platform, so nothing here treats it as stale" "stat $(mj_rel "$path")"
    return 2
  }
  if [ "$age" -lt "$MJ_RECOVER_AGE_SECS" ]; then
    printf '  live        %s — %s old, under the %s threshold; a run may still be using it\n' \
      "$(mj_rel "$path")" "$(mj_age_human $((age / 60)))" "$MJ_RECOVER_AGE"
    return 1
  fi
  return 0
}

mj_recover_orphans() {
  printf 'orphans — stray files in the record stores\n'
  local n=0 f store
  store="$(mj_session_store)"

  # 1. the publish temps of every record store. `mj_publish_record` makes them; only a
  # process that died between the mktemp and the rm leaves one.
  local d
  for d in "$store" "$MJ_STATE_DIR/checkpoints" "$MJ_STATE_DIR/handovers"; do
    [ -d "$d" ] || continue
    for f in "$d"/.tmp.*; do
      [ -f "$f" ] || continue
      n=$((n + 1))
      local rdy=0; mj_recover_stray_ready "$f" || rdy=$?
      case "$rdy" in 1) continue ;; 2) MJ_RECOVER_SKIPPED=$((${MJ_RECOVER_SKIPPED:-0} + 1)); continue ;; esac
      mj_recover_classify_temp "$f"
    done
  done

  # 2. the rename temps. Every one of them is `<target>.mj-tmp`, written and renamed in one
  # `&&`, so one that exists means the write failed or the process died mid-write.
  local rdy
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    n=$((n + 1))
    rdy=0; mj_recover_stray_ready "$f" || rdy=$?
    case "$rdy" in 1) continue ;; 2) MJ_RECOVER_SKIPPED=$((${MJ_RECOVER_SKIPPED:-0} + 1)); continue ;; esac
    if [ -e "${f%.mj-tmp}" ]; then
      printf '  orphan      %s — its target %s exists; the rename completed or was retried\n' \
        "$(mj_rel "$f")" "$(mj_rel "${f%.mj-tmp}")"
      MJ_RECOVER_ACTS=$((${MJ_RECOVER_ACTS:-0} + 1))
      [ "$MJ_RECOVER_CHECK" = 1 ] || rm -f "$f"
    else
      printf '  incomplete  %s — its target %s does not exist; a write that never finished\n' \
        "$(mj_rel "$f")" "$(mj_rel "${f%.mj-tmp}")"
      mj_warn recover "$(mj_rel "$f")" "an unfinished write whose target is absent; read it before removing it" "cat $(mj_rel "$f")"
      MJ_RECOVER_SKIPPED=$((${MJ_RECOVER_SKIPPED:-0} + 1))
    fi
  done < <(find "$MJ_STATE_DIR" "$store" -name '*.mj-tmp' -type f 2>/dev/null | LC_ALL=C sort)

  # 3. the site generator's staging directories at the repository root.
  for f in "$MJ_ROOT"/.mj-stage.*; do
    [ -e "$f" ] || continue
    n=$((n + 1))
    rdy=0; mj_recover_stray_ready "$f" || rdy=$?
    case "$rdy" in 1) continue ;; 2) MJ_RECOVER_SKIPPED=$((${MJ_RECOVER_SKIPPED:-0} + 1)); continue ;; esac
    printf '  orphan      %s — a staging directory of scripts/generate-site-data, %s old; the run that made it did not finish\n' \
      "$(mj_rel "$f")" "$(mj_age_human $(( $(mj_recover_age_secs "$f") / 60 )))"
    MJ_RECOVER_ACTS=$((${MJ_RECOVER_ACTS:-0} + 1))
    [ "$MJ_RECOVER_CHECK" = 1 ] || rm -rf "$f"
  done

  # 4. anything in the checkpoint store that is not a checkpoint. The store holds files
  # named by `mj_publish_record`, and nothing in this tool ever puts a directory in it: a
  # directory there was made by a person or an agent borrowing an ignored path as scratch
  # space. What it is is not this command's to guess, so it is named, measured, and left
  # exactly where it is.
  #
  # It is not inert while it sits there. `lib/doctor.sh` and `lib/finish.sh` count the
  # store with `find … -name '*.md'` and no `-maxdepth 1`, while every other reader —
  # `mj_resolve_latest`, `mj_record_list`, `mj_search_records`, and the Rust `count` — is
  # depth 1. So a stray directory holding Markdown inflates the retention figure doctor
  # judges against `checkpoint.retention_max_files` and the `checkpoints` count stamped
  # into the `task.finished` ledger line, and shows up nowhere a person would look for it.
  # That is reported here because this is the command that knows the directory is there.
  for f in "$MJ_STATE_DIR"/checkpoints/*/; do
    [ -d "$f" ] || continue
    n=$((n + 1))
    local md; md="$(find "$f" -name '*.md' 2>/dev/null | wc -l | tr -d ' ')"
    printf '  foreign     %s — a directory in the checkpoint store; %s, not written by this tool\n' \
      "$(mj_rel "${f%/}")" "$(mj_recover_size "${f%/}")"
    mj_info recover "$(mj_rel "${f%/}")" "not a checkpoint record; nothing here deletes it — move it under tmp/ by hand once you know what it is" "ls -la $(mj_rel "${f%/}")"
    [ "$md" = 0 ] || mj_warn recover "$(mj_rel "${f%/}")" "holds $md Markdown file(s), which doctor and finish count as checkpoints because they walk the store recursively while every other reader is depth 1" "find $(mj_rel "$MJ_STATE_DIR")/checkpoints -name '*.md' | wc -l"
  done

  # 5. the stores that look legacy and are not. Both of these were put to this command as
  # legacy directories to migrate; both have a living writer, and a store is legacy only
  # when nothing writes it any more — a question about the code, never about the dates on
  # the files. The finding they do carry is a different one: each has exactly one writer
  # and no reader at all, in this tool or in the executable. The report says both halves,
  # so that the next reader does not decide from the mtimes exactly as this command's own
  # brief did.
  mj_recover_report_store "$MJ_STATE_DIR/archive" \
    'written by lib/start.sh when a new task displaces a finished one; no reader anywhere'
  mj_recover_report_store "$MJ_STATE_DIR/completed" \
    'written by lib/finish.sh from --note; no reader anywhere'

  [ "$n" = 0 ] && printf '  no stray files\n'
  return 0
}

# A store that exists, with the writer that owns it. Nothing is done to it; the point is
# that the report says which stores have an owner, so that a later cleanup does not have to
# rediscover it from file dates.
mj_recover_report_store() {
  local d="$1" owner="$2"
  [ -d "$d" ] || return 0
  printf '  live        %s — %s file(s); %s\n' \
    "$(mj_rel "$d")" "$(find "$d" -type f 2>/dev/null | wc -l | tr -d ' ')" "$owner"
  return 0
}

mj_recover_size() {
  local n; n="$(du -sh "$1" 2>/dev/null | cut -f1)"
  if [ -n "$n" ]; then printf '%s in %s file(s)' "$n" "$(find "$1" -type f 2>/dev/null | wc -l | tr -d ' ')"
  else printf 'size unreadable'; fi
}

# One publish temp, by what is in it. Nothing is removed on the strength of the name: the
# file is read, and a temp that turns out to hold a record nobody published is published
# rather than deleted — that is the whole difference between recovery and a sweep.
mj_recover_classify_temp() {
  local f="$1" sid existing
  if [ ! -s "$f" ]; then
    printf '  orphan      %s — empty; a publish that died before it wrote anything\n' "$(mj_rel "$f")"
    MJ_RECOVER_ACTS=$((${MJ_RECOVER_ACTS:-0} + 1))
    [ "$MJ_RECOVER_CHECK" = 1 ] || rm -f "$f"
    return 0
  fi
  sid="$(sed -n 's/^session_id: //p' "$f" | head -n 1)"
  if [ -z "$sid" ]; then
    printf '  foreign     %s — has content but no session_id; not a record this version wrote\n' "$(mj_rel "$f")"
    mj_warn recover "$(mj_rel "$f")" "content this command cannot classify; read it before removing it" "cat $(mj_rel "$f")"
    MJ_RECOVER_SKIPPED=$((${MJ_RECOVER_SKIPPED:-0} + 1))
    return 0
  fi
  existing="$(grep -rl "^session_id: $sid\$" "$(mj_session_store)" --include='*.md' 2>/dev/null | head -n 1 || true)"
  if [ -n "$existing" ]; then
    printf '  orphan      %s — episode %s is published at %s; the link succeeded and the temp was not removed\n' \
      "$(mj_rel "$f")" "$sid" "$(mj_rel "$existing")"
    MJ_RECOVER_ACTS=$((${MJ_RECOVER_ACTS:-0} + 1))
    [ "$MJ_RECOVER_CHECK" = 1 ] || rm -f "$f"
    return 0
  fi
  printf '  incomplete  %s — holds the only copy of episode %s; publishing it\n' "$(mj_rel "$f")" "$sid"
  MJ_RECOVER_ACTS=$((${MJ_RECOVER_ACTS:-0} + 1))
  [ "$MJ_RECOVER_CHECK" = 1 ] && return 0
  local final
  final="$(mj_publish_record "$(mj_session_store)" "$sid" "$f")" \
    || { mj_err "recover: could not publish $(mj_rel "$f")"; return 1; }
  rm -f "$f"
  mj_ledger_append session.recovered \
    "\"session_id\":\"$(mj_json_esc "$sid")\",\"reason\":\"an unpublished record was found in a publish temp and published\",\"session_path\":\"$(mj_json_esc "${final#"$MJ_ROOT/"}")\""
  printf '              published as %s\n' "$(mj_rel "$final")"
  return 0
}
